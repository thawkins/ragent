//! YouTube transcript extraction for `mf_fetch`.
//!
//! When `mf_fetch` is asked to retrieve a YouTube watch URL, the normal HTML
//! extraction chain only recovers the page chrome and description. This module
//! parses the embedded `ytInitialPlayerResponse` JSON, discovers available
//! caption tracks, fetches the default (or first English) track, and returns a
//! clean transcript that can be used as the source body in research outputs.

use anyhow::{Context, Result, bail};
use quick_xml::Reader;
use quick_xml::events::Event;

use super::http;

/// Extract the transcript for a YouTube video from its watch-page HTML.
///
/// `html` is the raw HTML of `https://www.youtube.com/watch?v=<VIDEO_ID>`.
/// The function returns `(title, transcript)` where `title` is the video
/// title extracted from the page metadata and `transcript` is the full caption
/// text with timestamps.
///
/// # Errors
///
/// Returns an error when:
///
/// - `ytInitialPlayerResponse` cannot be located in the HTML.
/// - The player response contains no usable caption tracks.
/// - The caption track cannot be fetched or parsed.
///
/// # Examples
///
/// ```no_run
/// use ragent_tools_extended::masterfetch::youtube::extract_transcript_from_watch_page;
///
/// # async fn demo(html: &str) -> anyhow::Result<()> {
/// let (title, transcript) = extract_transcript_from_watch_page(html).await?;
/// # Ok(()) }
/// ```
pub async fn extract_transcript_from_watch_page(html: &str) -> Result<(String, String)> {
    let player_response = parse_yt_initial_player_response(html)?;
    let caption_url = caption_track_url(
        &player_response,
        player_response
            .get("videoDetails")
            .and_then(|v| v.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("YouTube video"),
    )?;

    let title = player_response
        .get("videoDetails")
        .and_then(|v| v.get("title"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("YouTube video")
        .to_string();

    let client =
        http::shared_client().context("failed to build HTTP client for YouTube captions")?;
    let captions_xml = client
        .get(&caption_url)
        .header("Accept-Language", "en-US,en")
        .send()
        .await
        .with_context(|| format!("failed to fetch YouTube captions from {caption_url}"))?
        .text()
        .await
        .with_context(|| format!("failed to read YouTube captions body from {caption_url}"))?;

    let transcript =
        parse_caption_xml(&captions_xml).with_context(|| "failed to parse YouTube caption XML")?;

    Ok((title, transcript))
}

/// Parse the `ytInitialPlayerResponse` object from a YouTube watch page.
///
/// YouTube embeds this as a JavaScript variable assignment near the top of the
/// HTML. We locate it with a simple regex and parse the JSON object.
pub fn parse_yt_initial_player_response(html: &str) -> Result<serde_json::Value> {
    let re = regex::Regex::new(r"ytInitialPlayerResponse\s*=\s*(\{.*?\});").expect("valid regex");
    let caps = re
        .captures(html)
        .context("ytInitialPlayerResponse not found in YouTube page")?;
    let json_str = caps
        .get(1)
        .context("ytInitialPlayerResponse match missing JSON")?
        .as_str();
    serde_json::from_str(json_str).context("failed to parse ytInitialPlayerResponse JSON")
}

/// Choose the best available caption track URL from the player response.
///
/// Preference order:
/// 1. The renderer's default caption track.
/// 2. The first English track (`languageCode == "en"`).
/// 3. The first track of any language.
pub fn caption_track_url(
    player_response: &serde_json::Value,
    _video_title: &str,
) -> Result<String> {
    let tracks = player_response
        .get("captions")
        .and_then(|c| c.get("captionTracks"))
        .and_then(|t| t.as_array())
        .context("no caption tracks available for this YouTube video")?;

    if tracks.is_empty() {
        bail!("no caption tracks available for this YouTube video");
    }

    let pick = tracks
        .iter()
        .find(|t| {
            t.get("isDefault")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .or_else(|| {
            tracks
                .iter()
                .find(|t| t.get("languageCode").and_then(|v| v.as_str()) == Some("en"))
        })
        .or_else(|| tracks.first())
        .context("no caption track could be selected")?;

    pick.get("baseUrl")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .context("selected caption track has no baseUrl")
}

/// Parse YouTube caption XML into a readable transcript string.
///
/// Each `<text>` element becomes one line prefixed with its start timestamp.
pub fn parse_caption_xml(xml: &str) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut transcript = String::new();
    let mut buf = Vec::new();
    let mut current_text = String::new();
    let mut current_start: Option<f64> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"text" => {
                for attr in e.attributes() {
                    let attr = attr?;
                    if attr.key.as_ref() == b"start"
                        && let Ok(start) = String::from_utf8_lossy(&attr.value).parse::<f64>()
                    {
                        current_start = Some(start);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = std::str::from_utf8(e.as_ref()).unwrap_or("");
                current_text.push_str(text);
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"text" => {
                let line = current_text.trim();
                if !line.is_empty() {
                    if let Some(start) = current_start {
                        let minutes = (start as u64) / 60;
                        let seconds = (start as u64) % 60;
                        transcript.push_str(&format!("[{minutes:02}:{seconds:02}] {line}\n"));
                    } else {
                        transcript.push_str(line);
                        transcript.push('\n');
                    }
                }
                current_text.clear();
                current_start = None;
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(transcript.trim().to_string())
}

/// Return `true` if the supplied URL points to a YouTube video watch page.
#[must_use]
pub fn is_youtube_url(url: &str) -> bool {
    url::Url::parse(url).ok().is_some_and(|u| {
        let host = u.host_str().unwrap_or("").to_ascii_lowercase();
        host.ends_with("youtube.com") || host == "youtu.be"
    })
}

/// Try to extract the video title from the watch-page HTML `<title>` tag as a
/// fallback when the player response is missing.
#[must_use]
pub fn fallback_title_from_html(html: &str) -> Option<String> {
    let re = regex::Regex::new(r"<title>(.*?)\s*-?\s*YouTube</title>").ok()?;
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|t| !t.is_empty())
}
