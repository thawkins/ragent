//! The one stream tool-call accumulator every provider family shares (FR-004).
//!
//! A provider stream names the tool call a frame belongs to in one of two ways,
//! and the family's [`ToolWireProfile::stream_keying`] says which:
//!
//! * [`StreamKeying::ByIndex`] - the provider supplies a content-block, choice or
//!   part index and every delta belongs to the call opened at that index
//!   (Anthropic, Bedrock, the OpenAI chat-completions family);
//! * [`StreamKeying::ByCallId`] - the provider repeats the call id on every frame
//!   (the Responses API, native Ollama).
//!
//! Before this module each provider module kept its own structure for that
//! bookkeeping, and the structures had drifted: `anthropic.rs` held a
//! `BTreeMap<u64, String>` of open blocks, the OpenAI-family parser in `sse.rs`
//! held a `HashMap<u64, String>` plus a `HashSet<u64>` of already-started indices,
//! `bedrock.rs` held a `HashMap<String, String>` on its Anthropic-shaped path and
//! a single-slot `String` on its Converse path, and `gemini.rs` held a `Vec` of
//! pending calls flushed at the finish frame. Four structures
//! meant four answers to "which call does this delta belong to", and the
//! single-slot answer is wrong the moment a response carries two tool calls
//! (FR-005).
//!
//! [`ToolCallAccumulator`] is that one answer. It is a *provider-side* tracker: it
//! decides which open call a frame belongs to and returns the canonical
//! [`StreamEvent`] naming the resolved id, so no provider invents a lookup of its
//! own. It does **not** keep the argument bytes of a streamed call: the consumer
//! (the agent loop) already concatenates fragments into its own bounded buffer,
//! and a second copy here would double the memory a large call takes for no
//! benefit (NFR-001). Fragments are forwarded verbatim, which is also what FR-007
//! requires - the provider layer must not decode them.
//!
//! The accumulator does hold a whole-argument call, but only one the provider
//! hands over in a single frame and asks to be deferred ([`defer`](ToolCallAccumulator::defer),
//! Gemini's `functionCall`): the string already exists, so keeping it copies
//! nothing that was not already allocated.
//!
//! The accumulator guarantees the pairing its callers need: the id a
//! [`ToolCallEnd`](crate::llm::StreamEvent::ToolCallEnd) carries is exactly the id
//! its [`ToolCallStart`](crate::llm::StreamEvent::ToolCallStart) carried, because
//! the id is pinned when the start is emitted, and exactly one end is produced per
//! started call. A frame that repeats a tool name cannot produce a second start.
//!
//! ## Every family keys a frame, and a frame that keys nothing is dropped
//!
//! There is no "it must mean the call I opened last" fallback. A frame that names
//! no open call is dropped with a `tracing::debug!` rather than folded onto
//! whichever call happens to be open, because that fold is exactly what merges two
//! parallel calls into one (FR-005): the Bedrock Converse path used to attribute
//! every argument fragment and every stop frame to the call it had opened most
//! recently, so a response carrying two `toolUse` blocks appended both blocks'
//! arguments to the second call - one call's input lost, the other's a
//! concatenation of two - and then ended that call more than once. Both Bedrock
//! paths now key strictly by the content-block index their frame carries -
//! `<index>` for the Anthropic Messages shape, `contentBlockIndex` for Converse -
//! and warn when a frame omits it (FUNC-032).
//!
//! The two helpers that once offered that guess are **deleted**, not merely left
//! unused: a `*_last` method that answers "the call I opened most recently" is
//! the FR-005 failure one method call away for the next provider that cannot find
//! its index, so no such method exists here for a caller to reach for. The
//! source scan in `tests/test_wire_decode.rs` fails if one comes back.
//!
//! ## A call the provider names no id for gets a synthetic one (FR-019)
//!
//! Gemini's `functionCall` never carries an id, native Ollama's `role:"tool"`
//! result has no id slot, and an OpenAI-compatible server may simply omit
//! `tool_calls[].id`. The accumulator then mints
//! `<provider>_<counter>_<name>` from the provider label it was constructed with
//! and a per-stream monotonic counter, and logs one `tracing::debug!` naming that
//! provider - so two id-less calls in one response are distinguishable and the
//! log says who left them unnamed.
//!
//! The name is *not* recovered from that id, although the id contains it for a
//! human reading a log: the name travels beside the id on every record
//! ([`ToolCallStart`](crate::llm::StreamEvent::ToolCallStart) carries `{ id, name }`
//! and the pending call it creates keeps both), so a tool whose name contains `_`
//! cannot be mis-read.
//!
//! ## An id the provider reuses inside one response is reconciled (FR-020)
//!
//! A stream may name two different calls with the same id - a local model asked to
//! give both calls the id `call_dup` will, and a server replaying a cached frame can
//! too. Both calls then reach the same history and the consumer keys its pending
//! calls by id, so the second one's start would overwrite the first's and only one
//! result would ever come back. The accumulator therefore keeps a ledger of every
//! id it has emitted a start under *in this stream*, and the first call to claim an
//! id keeps it while a later one is re-keyed to a freshly minted FR-019 id, with one
//! `tracing::warn!` naming the duplicated id. The warning is emitted at the moment
//! the collision is decided - when the second start is built - so a provider that
//! repeats the frame every step cannot multiply the log line.
//!
//! The ledger is per response, which is the right scope: the same provider id
//! appearing in the *next* turn's response is a different call against a different
//! request, and a replayed history carries both ids. A closed call stays in the
//! ledger for the same reason - two sequential calls in one response collide just as
//! two parallel ones do.

use super::{StreamKeying, ToolWireProfile};
use crate::llm::StreamEvent;

/// The key that identifies the tool call a stream frame belongs to (FR-004).
///
/// Which variant a provider passes is decided by what its frame supplies
/// ([`stream_key`]) and, for the fallback, by the family's [`StreamKeying`]; the
/// accumulator records the key so a later end frame can name the call an earlier
/// start frame opened, whatever order the frames arrive in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StreamKey {
    /// The content-block / choice / part index the provider supplies.
    Index(u64),
    /// The call id the provider supplies on the frame itself.
    CallId(String),
}

impl StreamKey {
    /// The key a frame addresses, from the index and the id that frame supplies.
    ///
    /// An index wins when the provider sends one (SSE content blocks, chat
    /// `choices[].delta.tool_calls[].index`); otherwise a non-empty id names the
    /// call (Bedrock Converse `contentBlockStart`, the Responses API). `None`
    /// means the frame names no call at all, and the caller decides whether it has
    /// a sensible fallback.
    #[must_use]
    pub fn from_frame(index: Option<u64>, id: Option<&str>) -> Option<Self> {
        match (index, id) {
            (Some(index), _) => Some(Self::Index(index)),
            (None, Some(id)) if !id.is_empty() => Some(Self::CallId(id.to_string())),
            _ => None,
        }
    }
}

/// The tool name a synthetic id ends with when the provider names no tool.
///
/// FR-019's id is `<provider>_<counter>_<name>`, so the last segment cannot be
/// empty: a frame that carries no name at all still gets a well-formed id, and
/// the `tracing::debug!` beside it records that the name was missing.
const UNNAMED_TOOL: &str = "unnamed";

/// One tool call the stream has opened and not yet ended.
#[derive(Debug)]
struct OpenCall {
    /// The key the provider addresses this call by (FR-004).
    key: StreamKey,
    /// The id the *provider* gave the call, byte for byte (FR-006). `None` until a
    /// frame carries one, which an index-keyed call may never do.
    ///
    /// Kept apart from [`started_id`](Self::started_id) on purpose: the provider's
    /// id and the id the start carried can differ when FR-020 re-keys a duplicate,
    /// and only the pinned start id may be repeated to the consumer.
    provider_id: Option<String>,
    /// Whether a frame (or [`set_id`](ToolCallAccumulator::set_id)) supplied that id,
    /// as opposed to [`position`](ToolCallAccumulator::position) seeding it from the
    /// call-id key so the call is addressable before any frame names it (FR-019).
    ///
    /// A key is not an id: a `CallId` key is the provider's id for the native shapes,
    /// but for an id-less call it is only a handle, and minting must not be skipped
    /// because the handle looks like one. A handle is written as `call@<counter>` -
    /// a spelling no provider uses - so a record of it reads as the placeholder it
    /// is (see the FR-019 source scan in `tests/test_wire_decode.rs`).
    key_is_id: bool,
    /// The tool name the provider reported, empty until a frame carries it.
    ///
    /// Held beside the id, never inside it: a caller that needs the name reads
    /// this field, so a tool called `read_file` cannot be mis-read as `read`
    /// (FR-019).
    name: String,
    /// The id the emitted `ToolCallStart` carried. `None` means no start has been
    /// emitted, so no end may be emitted either: the consumer pairs an end with the
    /// pending call its start created, and a start-less end would report a call as
    /// complete that was never dispatched.
    started_id: Option<String>,
}

/// A whole tool call a provider hands over in one frame, held until the provider
/// flushes its frame (Gemini's `functionCall` beside `finishReason`).
#[derive(Debug)]
struct DeferredCall {
    /// The id the accumulator minted for this call and emits on its start, delta
    /// and end (FR-019), e.g. `gemini_0_get_weather`.
    id: String,
    /// The tool name the provider reported, carried beside the id rather than
    /// inside it.
    name: String,
    /// The whole argument object, already serialised by the provider.
    args_json: String,
}

/// The shared, profile-keyed tracker for tool calls arriving on a provider stream
/// (FR-004).
///
/// Construct one per stream with the family's profile and the provider's own id,
/// then feed it the key each frame names. Every method returns the canonical event
/// to yield, or `None` when the frame produces no event, so a provider's stream
/// loop yields exactly the events it did before - resolved by one implementation
/// instead of four.
///
/// The provider id is an *input*, not a decoration: FR-019's synthetic id is
/// `<provider>_<counter>_<name>`, so an accumulator built without one could not
/// mint a conforming id, and the `tracing::debug!` the requirement asks for names
/// the same string.
///
/// # Examples
///
/// ```
/// use ragent_llm::llm::StreamEvent;
/// use ragent_llm::provider::wire::decode::{StreamKey, ToolCallAccumulator};
/// use ragent_llm::provider::wire::profile_table::ANTHROPIC_MESSAGES;
///
/// let mut acc = ToolCallAccumulator::new(ANTHROPIC_MESSAGES, "anthropic");
/// let key = StreamKey::Index(2);
/// assert!(matches!(
///     acc.open(key.clone(), Some("toolu_1"), "get_weather"),
///     Some(StreamEvent::ToolCallStart { id, name }) if id == "toolu_1" && name == "get_weather"
/// ));
/// assert_eq!(acc.open_count(), 1);
/// assert!(acc.delta(&key, "{\"city\":").is_some());
/// assert!(acc.delta(&key, "\"Paris\"}").is_some());
/// assert!(matches!(
///     acc.end(&key),
///     Some(StreamEvent::ToolCallEnd { id }) if id == "toolu_1"
/// ));
/// assert_eq!(acc.open_count(), 0);
/// // A second end for the same block produces nothing: exactly one end per call.
/// assert!(acc.end(&key).is_none());
/// ```
#[derive(Debug)]
pub struct ToolCallAccumulator {
    /// The keying the family declares; decides how [`end_all`](Self::end_all)
    /// orders the calls it closes.
    keying: StreamKeying,
    /// The provider id a synthesised call id is built from (FR-019), and the name
    /// the debug line carries when one is minted.
    provider: String,
    /// How many synthetic ids this stream has minted. Monotonic per stream, so two
    /// id-less calls in one response can never share an id.
    counter: u64,
    /// The calls this stream has opened, in the order it opened them. A response
    /// carries a handful of tool calls, so a linear scan is cheaper than a map and
    /// keeps the ordering the consumer needs (FR-005) without a second structure.
    open: Vec<OpenCall>,
    /// Every call id this stream has already emitted a start under, in emission
    /// order (FR-020).
    ///
    /// A closed call is *not* removed from this ledger: two calls in one response
    /// can arrive one after the other as easily as side by side, and both reach the
    /// same history, so "has this id been used in this response" is the question -
    /// not "is a call by this id open right now". A response carries a handful of
    /// calls, so a linear scan is again cheaper than a set.
    emitted_ids: Vec<String>,
    /// Calls a provider asked to hold until it flushes its frame, in order.
    deferred: Vec<DeferredCall>,
}

impl ToolCallAccumulator {
    /// Create an accumulator for one stream of the family `profile` describes,
    /// naming the `provider` whose frames it will read.
    ///
    /// `provider` is the provider id (`"gemini"`, `"ollama_cloud"`, ...); FR-019
    /// builds a synthetic call id from it, so a caller cannot leave it blank
    /// without saying so at the call site.
    #[must_use]
    pub fn new(profile: ToolWireProfile, provider: &str) -> Self {
        Self {
            keying: profile.stream_keying,
            provider: provider.to_string(),
            counter: 0,
            open: Vec::new(),
            emitted_ids: Vec::new(),
            deferred: Vec::new(),
        }
    }

    /// The keying this accumulator was built with (FR-004).
    #[must_use]
    pub fn keying(&self) -> StreamKeying {
        self.keying
    }

    /// How many calls the stream has announced with a start and not yet ended.
    ///
    /// This is the count FR-022 tests: a provider that reports a tool-use finish
    /// with zero such calls produced no usable tool call, and the step is treated
    /// as truncated. Deferred calls do not count until they are flushed, because
    /// until then the consumer has not seen a start.
    #[must_use]
    pub fn open_count(&self) -> usize {
        self.open
            .iter()
            .filter(|call| call.started_id.is_some())
            .count()
    }

    /// How many whole calls are waiting for the provider to flush its frame.
    ///
    /// Gemini uses this to skip a flush that would emit nothing.
    #[must_use]
    pub fn deferred_count(&self) -> usize {
        self.deferred.len()
    }

    /// Record the provider's id for the call at `key`, opening the call if this is
    /// the first frame to mention it.
    ///
    /// A frame may carry the id without a name (the OpenAI family does), so this
    /// emits nothing: it only makes the id available to the
    /// [`open`](Self::open) and [`delta`](Self::delta) calls that follow. An id
    /// that arrives after the start was emitted is ignored, so the start and end of
    /// a call can never disagree.
    pub fn set_id(&mut self, key: &StreamKey, id: &str) {
        if id.is_empty() {
            return;
        }
        let position = self.position(key.clone());
        self.store_id(position, id.to_string());
    }

    /// Write a provider-supplied id onto the call at `position`, and mark the call as
    /// one whose frames name it (FR-019).
    ///
    /// The mark is what separates the two kinds of `StreamKey::CallId` call: one a
    /// provider genuinely named, and one [`position`](Self::position) had to invent a
    /// key for because its frames carry no id at all. Only the second kind reaches
    /// FR-019's mint, and only this flag distinguishes them once the key and the id
    /// hold the same string.
    fn store_id(&mut self, position: usize, id: String) {
        let call = &mut self.open[position];
        call.provider_id = Some(id);
        call.key_is_id = true;
    }

    /// Announce the call at `key` with the provider's `id` (when the frame carries
    /// one) and its `name`, returning the `ToolCallStart` to yield.
    ///
    /// Returns `None` when this call's start has already been emitted: a provider
    /// that repeats the tool name on every argument frame then cannot produce a
    /// second start, which would push a second pending call with the same id onto
    /// the consumer.
    ///
    /// The id the start carries is the provider's own, the one FR-019 mints when the
    /// provider named none, or - when a *different* call in this response already
    /// claimed that id - a fresh synthetic id (FR-020).
    pub fn open(&mut self, key: StreamKey, id: Option<&str>, name: &str) -> Option<StreamEvent> {
        let position = self.position(key);
        if let Some(id) = id.filter(|id| !id.is_empty())
            && self.open[position].started_id.is_none()
        {
            self.store_id(position, id.to_string());
        }
        {
            let call = &mut self.open[position];
            if !name.is_empty() {
                call.name = name.to_string();
            }
            if call.started_id.is_some() {
                return None;
            }
        }
        let name = self.open[position].name.clone();
        // The id the provider asked for, or - when it named none - one minted
        // here (FR-019). The minted id is pinned on the call *before* the start
        // event is built, so every later frame about this call reads the same
        // string and start, delta and end cannot disagree.
        //
        // A key is not an id: the key only makes a call addressable, so a call whose
        // own frames carried no id - Gemini's `functionCall`, an Ollama
        // `tool_calls[]` entry without one, an OpenAI-compatible frame with an
        // index and no id - gets FR-019's id minted for it. `key_is_id` is how that
        // is told apart from a provider that named the call by its key.
        let id = match self.open[position].provider_id.clone() {
            Some(id) if self.open[position].key_is_id => {
                // FR-020: two calls in one response must not reach the consumer
                // under the same id, or the second result overwrites the first.
                self.start_id(id, &name)
            }
            // Either no id at all, or a key standing in for one: both are an
            // id-less call as far as FR-019 is concerned.
            Some(_) | None => {
                let minted = self.mint_for_unnamed(&name);
                self.emitted_ids.push(minted.clone());
                minted
            }
        };
        self.open[position].started_id = Some(id.clone());
        Some(StreamEvent::ToolCallStart { id, name })
    }

    /// The id a start for this call should carry, reconciling a duplicate id
    /// (FR-020).
    ///
    /// `requested` is the id the provider gave, or the one FR-019 minted. The first
    /// call in a response to claim an id keeps it; a later call that arrives under
    /// the same id is re-keyed to a freshly minted synthetic id, because the two
    /// calls reach the same history and the consumer keys its pending calls by id -
    /// the second would otherwise overwrite the first, and only one result would
    /// ever come back. One `tracing::warn!` names the duplicated id; it is logged
    /// here, once per collision, rather than per frame, because the collision is
    /// decided exactly once, when the second start is emitted.
    ///
    /// This is not reachable for a well-formed call-id-keyed family: where the
    /// family keys by call id (`StreamKeying::ByCallId`) the id *is* the key, so a
    /// repeated id names the same call and [`open`](Self::open) is idempotent on it -
    /// re-keying there would invent a call out of a repeated frame.
    fn start_id(&mut self, requested: String, name: &str) -> String {
        if !self.emitted_ids.contains(&requested) {
            self.emitted_ids.push(requested.clone());
            return requested;
        }
        // [`synthetic_id`](Self::synthetic_id) walks its counter past every id this
        // response has already used, so the replacement cannot itself collide.
        let replacement = self.synthetic_id(name);
        tracing::warn!(
            provider = %self.provider,
            duplicate_id = %requested,
            call_id = %replacement,
            tool = %name,
            "provider reused a tool-call id within one response; kept the first \
             call and re-keyed this one (FR-020)"
        );
        self.emitted_ids.push(replacement.clone());
        replacement
    }

    /// Attribute an argument fragment to the call open at `key`, returning the
    /// `ToolCallDelta` to yield.
    ///
    /// Returns `None` for an empty fragment, and for a key that no frame has
    /// mentioned: a delta that belongs to no call cannot be attributed to one
    /// (FR-004), and inventing a call for it would dispatch a tool the model never
    /// called.
    pub fn delta(&mut self, key: &StreamKey, args_json: &str) -> Option<StreamEvent> {
        if args_json.is_empty() {
            return None;
        }
        let Some(call) = self.open.iter_mut().find(|call| &call.key == key) else {
            tracing::debug!(
                key = ?key,
                "tool-call argument delta for a block no frame opened; dropping it"
            );
            return None;
        };
        // Only a call whose start the consumer saw can be named: the consumer
        // pairs deltas with the pending call that start created, so a delta for a
        // call that has not started is dropped with a debug line rather than sent
        // under an id nobody has seen.
        let Some(id) = call.started_id.clone() else {
            tracing::debug!(
                key = ?key,
                "tool-call argument delta for a call with no id yet; dropping it"
            );
            return None;
        };
        Some(StreamEvent::ToolCallDelta {
            id,
            args_json: args_json.to_string(),
        })
    }

    /// Close the call at `key`, returning the `ToolCallEnd` to yield.
    ///
    /// Returns `None` when the key names no open call, and when the call's start was
    /// never emitted - a block the consumer never saw cannot be reported as a
    /// completed call.
    pub fn end(&mut self, key: &StreamKey) -> Option<StreamEvent> {
        let position = self.open.iter().position(|call| &call.key == key)?;
        let call = self.open.remove(position);
        call.started_id.map(|id| StreamEvent::ToolCallEnd { id })
    }

    /// Close every open call, returning one `ToolCallEnd` per started call.
    ///
    /// [`StreamKeying::ByIndex`] families are closed in index order, so the ends of a
    /// parallel response arrive in the model's own order rather than in map
    /// iteration order; [`StreamKeying::ByCallId`] families keep the order the
    /// stream opened the calls in.
    pub fn end_all(&mut self) -> Vec<StreamEvent> {
        let mut open = std::mem::take(&mut self.open);
        if self.keying == StreamKeying::ByIndex {
            open.sort_unstable_by_key(|call| match &call.key {
                StreamKey::Index(index) => *index,
                // Unreachable for a `ByIndex` profile, whose callers only pass
                // `StreamKey::Index`; a mixed key sorts first rather than panicking.
                StreamKey::CallId(_) => 0,
            });
        }
        open.into_iter()
            .filter_map(|call| call.started_id.map(|id| StreamEvent::ToolCallEnd { id }))
            .collect()
    }

    /// Hold a whole tool call until [`flush_deferred`](Self::flush_deferred),
    /// minting the id it will carry (FR-019).
    ///
    /// Gemini delivers `functionCall` with its complete `args` object and must not
    /// emit it before it has read the `finishReason` that may arrive in the same
    /// frame, so the call is recorded here and flushed when the frame is handled.
    /// The caller passes the name the provider reported and nothing else: the id is
    /// the accumulator's to mint, so no provider invents its own spelling.
    pub fn defer(&mut self, name: String, args_json: String) {
        // The monotonic counter makes this id unique within the stream, so it cannot
        // collide with an earlier call; recording it keeps the ledger complete for a
        // provider id that happens to be spelled like a synthetic one (FR-020).
        let id = self.synthetic_id(&name);
        self.emitted_ids.push(id.clone());
        self.deferred.push(DeferredCall {
            id,
            name,
            args_json,
        });
    }

    /// Emit one start, one delta and one end per deferred call, in the order the
    /// provider delivered them, leaving nothing behind for a later flush to repeat.
    pub fn flush_deferred(&mut self) -> Vec<StreamEvent> {
        let deferred = std::mem::take(&mut self.deferred);
        let mut events = Vec::with_capacity(deferred.len() * 3);
        for call in deferred {
            events.push(StreamEvent::ToolCallStart {
                id: call.id.clone(),
                name: call.name,
            });
            events.push(StreamEvent::ToolCallDelta {
                id: call.id.clone(),
                args_json: call.args_json,
            });
            events.push(StreamEvent::ToolCallEnd { id: call.id });
        }
        events
    }

    /// The id an id-less call gets: `<provider>_<counter>_<name>` (FR-019).
    ///
    /// The counter is per stream and monotonic, so two calls a provider left
    /// unnamed in one response are still distinguishable; the name is included
    /// because a log line is read by a human, and it is the *record* - not this
    /// string - that the loop resolves a tool from. The id is also the one a
    /// duplicate is re-keyed to (FR-020), so the counter is what makes that
    /// replacement unique. This function only formats; the two callers log for
    /// their own reason, because "the provider named no id" and "the provider
    /// reused an id" are different stories.
    fn synthetic_id(&mut self, name: &str) -> String {
        let tool = if name.is_empty() { UNNAMED_TOOL } else { name };
        let mut id = format!("{}_{}_{tool}", self.provider, self.counter);
        self.counter += 1;
        // FR-020: a provider is free to spell its own id exactly like ours, so the
        // counter is walked past any id this response has already used. Without this
        // the mint itself would create the collision it exists to avoid.
        while self.emitted_ids.contains(&id) {
            id = format!("{}_{}_{tool}", self.provider, self.counter);
            self.counter += 1;
        }
        id
    }

    /// Mint the id an unnamed call gets and log the FR-019 debug line naming the
    /// provider that left it unnamed.
    fn mint_for_unnamed(&mut self, name: &str) -> String {
        let id = self.synthetic_id(name);
        tracing::debug!(
            provider = %self.provider,
            tool = %if name.is_empty() { UNNAMED_TOOL } else { name },
            call_id = %id,
            "provider returned a tool call with no id; synthesised one (FR-019)"
        );
        id
    }

    /// The position of the call at `key`, opening it if this is the first frame to
    /// mention the key.
    ///
    /// The key is what the provider *addresses* a call by; it is not an id, so a
    /// call opened here carries none until a frame supplies one, and [`open`](Self::open)
    /// mints FR-019's id for the ones that never do.
    fn position(&mut self, key: StreamKey) -> usize {
        if let Some(position) = self.open.iter().position(|call| call.key == key) {
            return position;
        }
        let provider_id = match &key {
            StreamKey::Index(_) => None,
            StreamKey::CallId(id) => Some(id.clone()),
        };
        self.open.push(OpenCall {
            key,
            provider_id,
            key_is_id: false,
            name: String::new(),
            started_id: None,
        });
        // The push above guarantees this index is in bounds.
        self.open.len() - 1
    }
}
