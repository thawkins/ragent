//! Inline tests for `local_gatherer.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

/// In-memory fake filesystem. Maps canonical paths to either file
/// contents or "directory" markers for `glob` lookups.
#[derive(Default)]
struct FakeFs {
    files: HashMap<PathBuf, String>,
    specs: HashMap<String, String>, // spec_id -> title
    grep_results: HashMap<PathBuf, Vec<GrepMatch>>,
    glob_calls: Mutex<Vec<String>>,
    grep_calls: Mutex<Vec<(PathBuf, Vec<String>)>>,
    read_calls: Mutex<Vec<PathBuf>>,
}

#[async_trait]
impl LocalTool for FakeFs {
    async fn glob(&self, root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
        self.glob_calls.lock().unwrap().push(pattern.to_string());
        // Match by file extension in the pattern, scoped to `root`.
        let ext = pattern.rsplit('.').next().unwrap_or("");
        let mut out: Vec<PathBuf> = self
            .files
            .keys()
            .filter(|p| p.extension().is_some_and(|e| e == ext) && p.starts_with(root))
            .cloned()
            .collect();
        out.sort();
        Ok(out)
    }

    async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>> {
        self.grep_calls
            .lock()
            .unwrap()
            .push((path.to_path_buf(), terms.to_vec()));
        if let Some(matches) = self.grep_results.get(path) {
            return Ok(matches.clone());
        }
        // Default: scan the file contents for any matching term.
        let Some(body) = self.files.get(path) else {
            return Ok(Vec::new());
        };
        let mut hits = Vec::new();
        for (i, line) in body.lines().enumerate() {
            let lower = line.to_lowercase();
            if terms.iter().any(|t| lower.contains(t)) {
                hits.push(GrepMatch {
                    line: i + 1,
                    text: line.to_string(),
                });
            }
        }
        Ok(hits)
    }

    async fn read(&self, path: &Path) -> anyhow::Result<String> {
        self.read_calls.lock().unwrap().push(path.to_path_buf());
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no fake file at {}", path.display()))
    }

    async fn list_specs(&self, _project_root: &Path) -> anyhow::Result<Vec<String>> {
        let mut ids: Vec<String> = self.specs.keys().cloned().collect();
        ids.sort();
        Ok(ids)
    }

    async fn spec_title(&self, _project_root: &Path, spec_id: &str) -> anyhow::Result<String> {
        Ok(self.specs.get(spec_id).cloned().unwrap_or_default())
    }
}

fn root() -> PathBuf {
    PathBuf::from("/project")
}

fn gatherer_with_fs(fs: FakeFs) -> (LocalGatherer, Arc<FakeFs>) {
    let arc = Arc::new(fs);
    (LocalGatherer::new(arc.clone()), arc)
}

#[tokio::test]
async fn gather_rejects_zero_max_sources() {
    let (g, _) = gatherer_with_fs(FakeFs::default());
    let cfg = LocalGatherConfig {
        max_local_sources: 0,
        ..LocalGatherConfig::default()
    };
    let err = g.gather(&root(), "topic", None, &cfg).await.unwrap_err();
    assert!(matches!(err, LocalGatherError::ZeroLimit));
}

#[tokio::test]
async fn gather_rejects_no_terms() {
    let (g, _) = gatherer_with_fs(FakeFs::default());
    let cfg = LocalGatherConfig {
        terms: Vec::new(),
        ..LocalGatherConfig::default()
    };
    let err = g.gather(&root(), "", None, &cfg).await.unwrap_err();
    assert!(matches!(err, LocalGatherError::NoTerms));
}

#[tokio::test]
async fn gather_falls_back_to_configured_terms_when_topic_is_empty() {
    let mut fs = FakeFs::default();
    let file = root().join("src/lib.rs");
    fs.files.insert(file.clone(), "alpha beta gamma\n".into());
    let arc = Arc::new(fs);
    let g = LocalGatherer::new(arc.clone());
    let cfg = LocalGatherConfig {
        terms: vec!["alpha".into()],
        ..LocalGatherConfig::default()
    };
    let sources = g.gather(&root(), "", None, &cfg).await.unwrap();
    assert!(!sources.is_empty(), "fallback terms should produce hits");
    let grep_calls = arc.grep_calls.lock().unwrap();
    assert!(
        grep_calls
            .iter()
            .any(|(_, terms)| terms.contains(&"alpha".to_string())),
        "grep should be called with the fallback term"
    );
}

#[tokio::test]
async fn gather_emits_local_sources_with_zero_padded_body_paths() {
    let mut fs = FakeFs::default();
    for name in &["alpha.rs", "beta.rs", "gamma.rs"] {
        let p = root().join("src").join(name);
        fs.files
            .insert(p, format!("this is the {name} file, alpha content\n"));
    }
    let (g, _) = gatherer_with_fs(fs);
    let cfg = LocalGatherConfig {
        terms: vec!["alpha".into()],
        max_local_sources: 5,
        ..LocalGatherConfig::default()
    };
    let sources = g
        .gather(&root(), "alpha content", None, &cfg)
        .await
        .unwrap();
    assert!(!sources.is_empty());
    for (i, src) in sources.iter().enumerate() {
        let Source::Local {
            body_path,
            kind,
            path,
            ..
        } = src
        else {
            panic!("expected Source::Local, got {src:?}");
        };
        assert_eq!(*kind, LocalSourceKind::InProject);
        assert_eq!(
            body_path.as_path(),
            PathBuf::from(format!("sources/local-{:02}.md", i + 1)).as_path()
        );
        assert!(
            std::path::Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("rs")),
            "path should end with .rs: {path}"
        );
    }
}

#[tokio::test]
async fn gather_caps_at_max_local_sources() {
    let mut fs = FakeFs::default();
    for i in 0..20 {
        let p = root().join(format!("file{i}.rs"));
        fs.files.insert(p, format!("file {i} contains alpha\n"));
    }
    let (g, _) = gatherer_with_fs(fs);
    let cfg = LocalGatherConfig {
        terms: vec!["alpha".into()],
        max_local_sources: 3,
        ..LocalGatherConfig::default()
    };
    let sources = g.gather(&root(), "alpha", None, &cfg).await.unwrap();
    let local_count = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    assert_eq!(local_count, 3);
}

#[tokio::test]
async fn gather_orders_higher_scores_first() {
    let mut fs = FakeFs::default();
    // file_high has 5 alpha hits, file_low has 1.
    fs.files.insert(
        root().join("high.rs"),
        "alpha\nalpha\nalpha\nalpha\nalpha\n".into(),
    );
    fs.files
        .insert(root().join("low.rs"), "alpha\nbeta\ngamma\n".into());
    let (g, _) = gatherer_with_fs(fs);
    let cfg = LocalGatherConfig {
        terms: vec!["alpha".into()],
        max_local_sources: 5,
        ..LocalGatherConfig::default()
    };
    let sources = g.gather(&root(), "alpha", None, &cfg).await.unwrap();
    let first = sources
        .iter()
        .find(|s| matches!(s, Source::Local { path, .. } if path.contains("high.rs")));
    assert!(first.is_some(), "highest-scoring file must come first");
    assert!(
        matches!(first.unwrap(), Source::Local { body_path, .. } if body_path == &PathBuf::from("sources/local-01.md"))
    );
}

#[tokio::test]
async fn gather_marks_extra_dir_files_with_extra_kind() {
    let mut fs = FakeFs::default();
    // In-project file.
    fs.files
        .insert(root().join("src/lib.rs"), "alpha content here\n".into());
    // Extra-dir file (will be globs'd from /extra).
    fs.files.insert(
        PathBuf::from("/extra/notes.md"),
        "alpha notes here\n".into(),
    );
    let (g, _) = gatherer_with_fs(fs);
    let cfg = LocalGatherConfig {
        terms: vec!["alpha".into()],
        max_local_sources: 10,
        ..LocalGatherConfig::default()
    };
    let sources = g
        .gather(&root(), "alpha", Some(Path::new("/extra")), &cfg)
        .await
        .unwrap();
    let kinds: Vec<LocalSourceKind> = sources
        .iter()
        .filter_map(|s| match s {
            Source::Local { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();
    assert!(kinds.contains(&LocalSourceKind::InProject));
    assert!(
        kinds.contains(&LocalSourceKind::Extra),
        "extra-dir matches must be tagged LocalSourceKind::Extra"
    );
}

#[tokio::test]
async fn gather_includes_spec_sources_with_relevance_notes() {
    let mut fs = FakeFs::default();
    fs.specs
        .insert("auth-refactor".into(), "Auth refactor plan".into());
    fs.specs
        .insert("model-router".into(), "Model router provider".into());
    let (g, _) = gatherer_with_fs(fs);
    let cfg = LocalGatherConfig {
        terms: vec!["auth".into()],
        max_local_sources: 5,
        ..LocalGatherConfig::default()
    };
    let sources = g.gather(&root(), "auth", None, &cfg).await.unwrap();
    let spec_sources: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .collect();
    assert_eq!(spec_sources.len(), 2);
    if let Source::Spec { relevance, .. } = spec_sources[0] {
        assert!(
            relevance.contains("Auth refactor plan") || relevance.contains("Model router"),
            "relevance should include the spec title: {relevance}"
        );
    }
}

#[tokio::test]
async fn gather_skips_spec_sources_when_skip_specs_is_true() {
    let mut fs = FakeFs::default();
    fs.specs
        .insert("auth-refactor".into(), "Auth refactor plan".into());
    fs.specs
        .insert("model-router".into(), "Model router provider".into());
    fs.files.insert(
        root().join("README.md"),
        "authentication notes here\n".into(),
    );
    let (g, _) = gatherer_with_fs(fs);
    let cfg = LocalGatherConfig {
        terms: vec!["auth".into()],
        max_local_sources: 5,
        skip_specs: true,
        ..LocalGatherConfig::default()
    };
    let sources = g.gather(&root(), "auth", None, &cfg).await.unwrap();
    let spec_count = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    assert_eq!(
        spec_count, 0,
        "spec sources should be omitted when skip_specs=true"
    );
    // Local sources should still be present.
    let local_count = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    assert!(local_count >= 1);
}
#[tokio::test]
async fn gather_returns_empty_when_no_files_match() {
    let mut fs = FakeFs::default();
    fs.files.insert(
        root().join("README.md"),
        "no matching keywords here\n".into(),
    );
    let (g, _) = gatherer_with_fs(fs);
    let cfg = LocalGatherConfig {
        terms: vec!["zzznotpresent".into()],
        max_local_sources: 5,
        ..LocalGatherConfig::default()
    };
    let sources = g
        .gather(&root(), "zzznotpresent", None, &cfg)
        .await
        .unwrap();
    let local_count = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    assert_eq!(local_count, 0);
}

#[test]
fn derive_terms_lowercases_dedupes_and_filters_short_tokens() {
    let terms = derive_terms("Async Rust async tokio A I", &[]);
    assert!(terms.contains(&"async".to_string()));
    assert!(terms.contains(&"rust".to_string()));
    assert!(terms.contains(&"tokio".to_string()));
    assert!(!terms.iter().any(|t| t.len() < 2));
    // Ensure dedup - "async" appears twice in input.
    assert_eq!(terms.iter().filter(|t| *t == "async").count(), 1);
}

#[test]
fn derive_terms_falls_back_when_topic_is_empty() {
    let fallback = vec!["foo".into(), "bar".into()];
    let terms = derive_terms("", &fallback);
    assert_eq!(terms, vec!["bar".to_string(), "foo".to_string()]);
}

#[test]
fn derive_terms_filters_short_tokens_from_fallback_too() {
    let fallback = vec!["x".into(), "yy".into(), "z".into()];
    let terms = derive_terms("", &fallback);
    // Single-char tokens filtered even from fallback.
    assert_eq!(terms, vec!["yy".to_string()]);
}

#[test]
fn derive_terms_strips_punctuation_and_splits_internal_punctuation() {
    let terms = derive_terms("async/await, tokio!", &[]);
    assert_eq!(
        terms,
        vec![
            "async".to_string(),
            "await".to_string(),
            "tokio".to_string()
        ]
    );
}

#[test]
fn derive_terms_keeps_apostrophes_in_contractions() {
    let terms = derive_terms("don't split this", &[]);
    assert!(terms.contains(&"don't".to_string()));
}

#[tokio::test]
async fn gather_specs_filters_by_terms_and_falls_back_when_narrow() {
    let mut fs = FakeFs::default();
    fs.specs
        .insert("auth-refactor".into(), "Sign-in Refactor".into());
    fs.specs
        .insert("auth-service".into(), "Auth Service".into());
    fs.specs.insert("auth-db".into(), "Auth Database".into());
    fs.specs
        .insert("model-router".into(), "Model Router".into());
    let g = LocalGatherer::new(Arc::new(fs));
    let root = root();

    // Three or more relevant specs keep the filter and exclude unrelated specs.
    let relevant = g
        .gather_specs(&root, &["auth".into()], 10, DEFAULT_LOCAL_CONCURRENCY)
        .await;
    assert_eq!(relevant.len(), 3);
    let ids: Vec<String> = relevant
        .iter()
        .map(|s| {
            if let Source::Spec { spec_id, .. } = s {
                spec_id.clone()
            } else {
                panic!("expected spec source")
            }
        })
        .collect();
    assert!(ids.contains(&"auth-refactor".to_string()));
    assert!(ids.contains(&"auth-service".to_string()));
    assert!(ids.contains(&"auth-db".to_string()));
    assert!(!ids.contains(&"model-router".to_string()));

    // A very narrow filter falls back to all specs so research still has
    // useful cross-references.
    let fallback = g
        .gather_specs(&root, &["zzzz".into()], 10, DEFAULT_LOCAL_CONCURRENCY)
        .await;
    assert_eq!(fallback.len(), 4);
}

#[test]
fn local_body_path_zero_pads_and_uses_one_based_index() {
    assert_eq!(local_body_path(0), PathBuf::from("sources/local-01.md"));
    assert_eq!(local_body_path(9), PathBuf::from("sources/local-10.md"));
    assert_eq!(local_body_path(99), PathBuf::from("sources/local-100.md"));
}

#[test]
fn collect_matched_terms_returns_dedup_lowercased_terms() {
    let matches = vec![
        GrepMatch {
            line: 1,
            text: "Async Rust is great".to_string(),
        },
        GrepMatch {
            line: 2,
            text: "tokio async runtime".to_string(),
        },
    ];
    let terms = vec!["async".into(), "RUST".into(), "missing".into()];
    let got = collect_matched_terms(&matches, &terms);
    // Order is first-seen; "async" appears twice (line 1 + 2) but is deduped.
    assert_eq!(got, vec!["async".to_string(), "rust".to_string()]);
}

#[test]
fn build_relevance_note_includes_matched_terms_and_first_line_snippet() {
    let matches = vec![
        GrepMatch {
            line: 1,
            text: "pub async fn main() { ... }".to_string(),
        },
        GrepMatch {
            line: 7,
            text: "let runtime = tokio::runtime::Runtime::new();".to_string(),
        },
    ];
    let note = build_relevance_note(&["async".into(), "tokio".into()], &matches);
    assert!(note.contains("2 match(es)"));
    assert!(note.contains("async, tokio"));
    assert!(note.contains("pub async fn main()"));
}

#[test]
fn build_relevance_note_truncates_long_matched_term_lists() {
    let matched: Vec<String> = (0..10).map(|i| format!("term{i}")).collect();
    let matches = vec![GrepMatch {
        line: 1,
        text: "term0 term1 term2".to_string(),
    }];
    let note = build_relevance_note(&matched, &matches);
    assert!(note.contains("(+7)"));
}

#[test]
fn build_local_excerpt_emits_match_lines_with_markers_and_omits_header() {
    let body = "\
header line one
header line two
fn async_main() {}
header line four
let runtime = tokio::new();
";
    let matches = vec![
        GrepMatch {
            line: 3,
            text: "fn async_main() {}".to_string(),
        },
        GrepMatch {
            line: 5,
            text: "let runtime = tokio::new();".to_string(),
        },
    ];
    let out = build_local_excerpt(body, &matches, 30);
    // Header is emitted at the top of the excerpt body.
    assert!(out.contains("Excerpt ->"));
    // Match lines are marked with the > glyph.
    assert!(out.contains('>'));
    // Context lines (one either side) are emitted with the space marker.
    assert!(out.contains("header line two"));
    assert!(out.contains("header line four"));
}

#[test]
fn build_local_excerpt_truncates_at_max_lines() {
    let body: String = (1..=100)
        .map(|i| format!("match line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let matches: Vec<GrepMatch> = (1..=100)
        .map(|i| GrepMatch {
            line: i,
            text: format!("match line {i}"),
        })
        .collect();
    let out = build_local_excerpt(&body, &matches, 5);
    // Only 5 lines of excerpt plus the trailing ellipsis marker should be
    // present.
    assert!(out.contains("5 more match(es) elided"));
    assert!(!out.contains("match line 6"));
}

#[test]
fn build_local_excerpt_handles_empty_match_list() {
    let body = "fn main() {}";
    let out = build_local_excerpt(body, &[], 30);
    assert_eq!(out, "");
}
