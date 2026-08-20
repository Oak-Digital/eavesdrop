//! The process boundary.
//!
//! These mirror the app's `models` types field for field. They are duplicated
//! rather than shared because a common crate would put llama.cpp back in the
//! app's dependency graph, which is the entire thing this binary exists to
//! avoid. `tests::the_wire_contract_matches_the_app` in the app's
//! `summarization` module round-trips real payloads through both definitions,
//! so a drift shows up as a failing test rather than a runtime parse error.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub transcript: Transcript,
    pub model_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub text: String,
    pub language: Option<String>,
    pub created_at: String,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub suggested_title: String,
    pub overview: String,
    pub key_points: Vec<String>,
    pub decisions: Vec<String>,
    pub action_items: Vec<String>,
    pub model: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SummarizationStage {
    Loading,
    Analyzing,
    Writing,
}

/// One per line on stdout. `progress` repeats; exactly one `summary` or
/// `error` ends the stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    Progress {
        stage: SummarizationStage,
        progress: f32,
    },
    Summary(Box<Summary>),
    Error {
        message: String,
    },
}
