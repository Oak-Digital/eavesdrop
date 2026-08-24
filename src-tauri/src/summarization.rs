//! Local meeting summaries.
//!
//! The model catalog and its downloads live here; the engine that runs them
//! lives in the `eavesdrop-summarizer` binary beside the app. That split is not
//! organizational — llama.cpp and whisper.cpp each vendor their own copy of
//! ggml, and linking both into one executable makes every `ggml_*` symbol
//! ambiguous. Keeping llama.cpp in its own process also means a `ggml_abort`,
//! which is how llama.cpp reports an allocation it cannot satisfy, ends a
//! summary instead of ending a recording.
//!
//! Nothing here reaches the network except a model download, and the engine
//! reaches nothing at all.

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

use crate::{
    download::{self, Checksum},
    error::{AppError, AppResult},
    models::{SummarizationStage, Summary, SummaryModelInfo, Transcript},
};

/// Matches `summarizer/src/wire.rs`. Both sides are plain serde structs over
/// the same field names; `tests::the_engine_protocol_round_trips` pins the
/// shape so a rename here fails a test rather than a summary.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Request<'a> {
    transcript: &'a Transcript,
    model_path: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Event {
    Progress {
        stage: SummarizationStage,
        progress: f32,
    },
    Summary(Box<Summary>),
    Error {
        message: String,
    },
}

struct ModelSpec {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    file: &'static str,
    url: &'static str,
    size_bytes: u64,
    sha256: &'static str,
}

/// Apache-2.0 instruct models, so a summary never depends on a licence the user
/// did not agree to. Both handle the European languages meetings here are held
/// in; the larger one writes noticeably tidier action items.
const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "qwen2.5-1.5b",
        name: "Compact",
        description: "Qwen2.5 1.5B — fast on any Mac, good enough for short meetings",
        file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
        size_bytes: 1_117_320_736,
        sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
    },
    ModelSpec {
        id: "qwen2.5-7b",
        name: "Detailed",
        description: "Qwen2.5 7B — sharper decisions and action items, noticeably slower",
        file: "Qwen2.5-7B-Instruct-Q4_K_M.gguf",
        url: "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
        size_bytes: 4_683_074_240,
        sha256: "65b8fcd92af6b4fefa935c625d1ac27ea29dcb6ee14589c55a8f115ceaaa1423",
    },
];

pub fn available_models(models_dir: &Path) -> Vec<SummaryModelInfo> {
    MODELS
        .iter()
        .map(|model| SummaryModelInfo {
            id: model.id.into(),
            name: model.name.into(),
            description: model.description.into(),
            size_bytes: model.size_bytes,
            installed: models_dir.join(model.file).is_file(),
        })
        .collect()
}

pub fn install_model(
    models_dir: &Path,
    model_id: &str,
    on_progress: impl FnMut(u64, u64),
) -> AppResult<PathBuf> {
    let model = MODELS
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| AppError::State("unknown summary model".into()))?;
    download::install(
        model.url,
        &models_dir.join(model.file),
        model.size_bytes,
        Checksum::Sha256(model.sha256),
        "summary model",
        on_progress,
    )
}

pub fn remove_model(models_dir: &Path, model_id: &str) -> AppResult<PathBuf> {
    let model = MODELS
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| AppError::State("unknown summary model".into()))?;
    let path = models_dir.join(model.file);
    if !path.is_file() {
        return Err(AppError::State(
            "this summary model is not installed".into(),
        ));
    }
    fs::remove_file(&path)?;
    let pending = path.with_extension("part");
    if pending.exists() {
        let _ = fs::remove_file(pending);
    }
    Ok(path)
}

/// The engine binary sits next to the app binary: Tauri copies `externalBin`
/// entries into the bundle alongside the executable, and a plain `cargo build`
/// leaves both in the same target directory, so one rule covers dev and
/// release.
fn engine_path() -> AppResult<PathBuf> {
    let executable = std::env::current_exe()
        .map_err(|error| AppError::Other(format!("could not locate the app: {error}")))?;
    let directory = executable
        .parent()
        .ok_or_else(|| AppError::Other("the app has no containing directory".into()))?;
    let name = if cfg!(windows) {
        "eavesdrop-summarizer.exe"
    } else {
        "eavesdrop-summarizer"
    };
    let path = directory.join(name);
    if !path.is_file() {
        return Err(AppError::Other(format!(
            "the summary engine is missing from {}",
            directory.display()
        )));
    }
    Ok(path)
}

/// Summarizes `transcript` with the model at `model_path`, reporting
/// `(stage, 0.0..=1.0)` as the engine works.
///
/// The engine is a child process, so its failures arrive as a message on the
/// wire or as a non-zero exit. An exit with no `error` event is the interesting
/// case: that is llama.cpp aborting under us, and the recording it was called
/// from carries on.
pub fn summarize(
    transcript: &Transcript,
    model_path: &Path,
    on_progress: &mut dyn FnMut(SummarizationStage, f32),
) -> AppResult<Summary> {
    let engine = engine_path()?;
    let model_path = model_path
        .to_str()
        .ok_or_else(|| AppError::State("the model path is not valid UTF-8".into()))?;
    let request = serde_json::to_string(&Request {
        transcript,
        model_path,
    })
    .map_err(|error| AppError::Other(format!("could not build the summary request: {error}")))?;

    let mut child = Command::new(&engine)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| AppError::Other(format!("could not start the summary engine: {error}")))?;

    // The engine reads stdin to EOF before it emits anything, so the write has
    // to finish and the pipe has to close before there is any output to read.
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::Other("the summary engine took no input".into()))?;
        stdin
            .write_all(request.as_bytes())
            .map_err(|error| AppError::Other(format!("could not send the transcript: {error}")))?;
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Other("the summary engine produced no output".into()))?;
    let mut summary = None;
    let mut failure = None;
    for line in BufReader::new(stdout).lines() {
        let line =
            line.map_err(|error| AppError::Other(format!("could not read the summary: {error}")))?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Event>(&line) {
            Ok(Event::Progress { stage, progress }) => on_progress(stage, progress.clamp(0.0, 1.0)),
            Ok(Event::Summary(value)) => summary = Some(*value),
            Ok(Event::Error { message }) => failure = Some(message),
            // A malformed line is the engine misbehaving rather than the
            // summary failing, so keep reading: the run may still end in a
            // usable summary or a proper error.
            Err(_) => continue,
        }
    }

    let status = child
        .wait()
        .map_err(|error| AppError::Other(format!("the summary engine did not finish: {error}")))?;
    if let Some(message) = failure {
        return Err(AppError::Other(message));
    }
    match summary {
        Some(summary) => Ok(summary),
        None if status.success() => Err(AppError::Other(
            "the summary engine finished without producing a summary".into(),
        )),
        None => Err(AppError::Other(format!(
            "the summary engine stopped unexpectedly ({status}) — the model may need more memory \
             than this computer has"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_catalog_detects_downloaded_models() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("qwen2.5-1.5b-instruct-q4_k_m.gguf"),
            b"model",
        )
        .unwrap();

        let models = available_models(temp.path());

        assert_eq!(models.len(), MODELS.len());
        assert!(
            models
                .iter()
                .find(|model| model.id == "qwen2.5-1.5b")
                .unwrap()
                .installed
        );
        assert!(
            !models
                .iter()
                .find(|model| model.id == "qwen2.5-7b")
                .unwrap()
                .installed
        );
        assert!(MODELS.iter().all(|model| model.sha256.len() == 64));
    }


    #[test]
    fn removing_a_model_updates_the_catalog() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("qwen2.5-1.5b-instruct-q4_k_m.gguf");
        std::fs::write(&path, b"model").unwrap();

        assert_eq!(remove_model(temp.path(), "qwen2.5-1.5b").unwrap(), path);
        assert!(
            !available_models(temp.path())
                .into_iter()
                .find(|model| model.id == "qwen2.5-1.5b")
                .unwrap()
                .installed
        );
    }

    /// Pins the wire shape against `summarizer/src/wire.rs`. A field renamed on
    /// either side stops matching this literal.
    #[test]
    fn the_engine_protocol_round_trips() {
        let event: Event =
            serde_json::from_str(r#"{"type":"progress","stage":"analyzing","progress":0.5}"#)
                .unwrap();
        assert!(matches!(
            event,
            Event::Progress {
                stage: SummarizationStage::Analyzing,
                ..
            }
        ));

        let event: Event = serde_json::from_str(
            r#"{"type":"summary","suggestedTitle":"T","overview":"O","keyPoints":["k"],
                "decisions":["d"],"actionItems":["a"],"model":"m","createdAt":"now"}"#,
        )
        .unwrap();
        let Event::Summary(summary) = event else {
            panic!("expected a summary event");
        };
        assert_eq!(summary.suggested_title, "T");
        assert_eq!(summary.action_items, ["a"]);

        let event: Event = serde_json::from_str(r#"{"type":"error","message":"boom"}"#).unwrap();
        assert!(matches!(event, Event::Error { message } if message == "boom"));

        // The request side is what the engine parses.
        let transcript = Transcript {
            text: "hello".into(),
            language: Some("english".into()),
            created_at: "now".into(),
            segments: Vec::new(),
        };
        let json = serde_json::to_string(&Request {
            transcript: &transcript,
            model_path: "/models/m.gguf",
        })
        .unwrap();
        assert!(json.contains("\"modelPath\":\"/models/m.gguf\""));
        assert!(json.contains("\"transcript\""));
    }
}
