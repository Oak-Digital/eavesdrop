//! The summary engine, in its own process.
//!
//! llama.cpp and whisper.cpp each vendor a complete copy of ggml. Linked into
//! one binary their symbols collide, which MSVC rejects outright and Apple's
//! linker resolves by silently keeping one copy — leaving two divergent ggml
//! versions sharing an address space. Running the summary engine as its own
//! executable means each process links exactly one ggml.
//!
//! It also contains the blast radius. ggml calls `abort()` on an allocation
//! failure or a failed assertion, so in-process a 7B model on a machine that
//! cannot hold it would take the recorder down mid-recording. Here it takes
//! down only this process, and the app reports a failed summary.
//!
//! Protocol: one JSON request on stdin, newline-delimited JSON events on
//! stdout — `progress` while working, then exactly one `summary` or `error`.

use std::{
    io::{Read, Write},
    num::NonZeroU32,
    path::Path,
    sync::OnceLock,
};

use chrono::Utc;
use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel, params::LlamaModelParams},
    sampling::LlamaSampler,
};

mod wire;

use wire::{Event, Request, SummarizationStage, Summary, Transcript};

/// The engine reports failure as a message rather than a typed error: the app
/// turns whatever arrives into its own `AppError`, so a second error taxonomy
/// here would only have to be translated back.
type EngineResult<T> = Result<T, String>;

/// Context window each pass runs in. Big enough for a transcript slice plus its
/// answer, small enough that the KV cache stays modest on a laptop.
const CONTEXT_TOKENS: u32 = 8192;
/// Transcript tokens handed to a single map pass.
const CHUNK_TOKENS: usize = 3_000;
/// Tokens the model may spend on the notes for one slice.
const NOTES_TOKENS: usize = 400;
/// Tokens the model may spend on the final summary.
const SUMMARY_TOKENS: usize = 800;
/// llama.cpp decodes the prompt in slices of this many tokens.
const BATCH_TOKENS: usize = 512;
/// llama.cpp's backend is process-wide and may only be initialized once, so it
/// outlives any single summary run.
fn backend() -> EngineResult<&'static LlamaBackend> {
    static BACKEND: OnceLock<Option<LlamaBackend>> = OnceLock::new();
    BACKEND
        .get_or_init(|| {
            LlamaBackend::init().ok().map(|mut backend| {
                // llama.cpp is chatty on stderr and none of it is ours to show.
                backend.void_logs();
                backend
            })
        })
        .as_ref()
        .ok_or_else(|| "could not start the local summary engine".to_string())
}

/// Summarizes `transcript` with the model at `model_path`, reporting
/// `(stage, 0.0..=1.0)` as it goes. Deliberately free of app plumbing so the
/// pipeline can be exercised from `examples/summary_probe.rs`.
pub fn summarize(
    transcript: &Transcript,
    model_path: &Path,
    custom_prompt: Option<&str>,
    on_progress: &mut dyn FnMut(SummarizationStage, f32),
) -> EngineResult<Summary> {
    if !model_path.is_file() {
        return Err(format!(
            "summary model was not found at {}",
            model_path.display()
        ));
    }
    let source = transcript.text.trim();
    if source.is_empty() {
        return Err("transcribe this recording before summarizing it".to_string());
    }

    on_progress(SummarizationStage::Loading, 0.0);
    let backend = backend()?;
    let threads = std::thread::available_parallelism()
        .map(|count| count.get().saturating_sub(1).clamp(1, 8))
        .unwrap_or(2) as i32;
    let model = LlamaModel::load_from_file(
        backend,
        model_path,
        &LlamaModelParams::default().with_n_gpu_layers(u32::MAX),
    )
    .map_err(|error| format!("could not load the summary model: {error}"))?;
    let template = model
        .chat_template(None)
        .map_err(|_| "this model has no chat template and cannot summarize".to_string())?;

    let language = language_instruction(transcript.language.as_deref());
    let chunks = split_into_chunks(&model, source);
    // Map passes plus the single reduce pass that writes the summary.
    let total_passes = chunks.len().max(1) + usize::from(chunks.len() > 1);
    let mut completed_passes = 0usize;

    let from_notes = chunks.len() > 1;
    let material = if from_notes {
        let mut notes = Vec::with_capacity(chunks.len());
        for (index, chunk) in chunks.iter().enumerate() {
            let pass = completed_passes;
            let text = generate(
                &model,
                backend,
                &template,
                NOTES_SYSTEM,
                &notes_prompt(index, chunks.len(), &language, chunk, custom_prompt),
                NOTES_TOKENS,
                threads,
                |ratio| {
                    on_progress(
                        SummarizationStage::Analyzing,
                        (pass as f32 + ratio) / total_passes as f32,
                    );
                },
            )?;
            notes.push(text);
            completed_passes += 1;
        }
        notes.join("\n")
    } else {
        chunks.into_iter().next().unwrap_or_default()
    };

    let pass = completed_passes;
    let answer = generate(
        &model,
        backend,
        &template,
        SUMMARY_SYSTEM,
        &summary_prompt(&language, &material, from_notes, custom_prompt),
        SUMMARY_TOKENS,
        threads,
        |ratio| {
            on_progress(
                SummarizationStage::Writing,
                (pass as f32 + ratio) / total_passes as f32,
            );
        },
    )?;
    on_progress(SummarizationStage::Writing, 1.0);

    let mut summary = parse_summary(&answer);
    if summary.overview.is_empty() && summary.key_points.is_empty() {
        return Err("the summary model returned nothing usable — try a larger model".to_string());
    }
    summary.model = model_file_name(model_path);
    summary.created_at = Utc::now().to_rfc3339();
    Ok(summary)
}

const NOTES_SYSTEM: &str = "You take notes on meeting transcripts. The transcript comes from \
automatic speech recognition, so expect misheard words and filler. Report only what the \
transcript actually says and never invent names, numbers, or commitments. Write about the \
meeting itself: never mention the transcript, the recording, or these instructions. Reply with \
bullet points and no preamble.";

const SUMMARY_SYSTEM: &str = "You summarize meetings. Report only what the material actually \
says and never invent names, numbers, or commitments. Write about the meeting itself: never \
mention the transcript, the notes, or these instructions. Reply using exactly the requested \
sections, with no preamble and no closing remarks.";

fn notes_prompt(
    index: usize,
    total: usize,
    language: &str,
    chunk: &str,
    custom_prompt: Option<&str>,
) -> String {
    let custom = custom_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\n\nWhen choosing what to retain for the final summary, follow these user instructions:\n{value}"))
        .unwrap_or_default();
    format!(
        "Minutes {} of {} from one meeting:\n\n{chunk}\n\nList the topics discussed, any \
decisions reached, and any tasks assigned along with who owns them. Keep every bullet to one \
short sentence, and write each one so it still makes sense on its own. {language}{custom}",
        index + 1,
        total
    )
}

fn summary_prompt(
    language: &str,
    material: &str,
    from_notes: bool,
    custom_prompt: Option<&str>,
) -> String {
    let source = if from_notes {
        "Notes from one meeting"
    } else {
        "A meeting"
    };
    let instructions = custom_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Summarize the meeting clearly and concisely. Focus on the topics discussed, decisions reached, and tasks assigned, including who owns them.");
    format!(
        "{source}:\n\n{material}\n\nUser instructions for the summary:\n{instructions}\n\nWrite the result using exactly this layout:\n\n\
TITLE: at most eight words naming what this meeting was actually about\n\
OVERVIEW: two or three sentences on what happened\n\
KEY POINTS:\n- one short sentence per topic that was discussed\n\
DECISIONS:\n- one short sentence per decision the meeting settled\n\
ACTION ITEMS:\n- one short sentence per task someone agreed to do, naming who owns it\n\n\
Every section must appear, with \"None\" under any section the meeting did not cover. Put each \
point under one heading only: a task someone took on belongs under ACTION ITEMS, not under KEY \
POINTS. The title names this meeting's subject, so do not use filler words like \"meeting\", \
\"sync\", \"discussion\", or \"update\". {language}"
    )
}

/// Whisper reports a language name such as "danish"; steering the model with it
/// keeps a Danish meeting from coming back summarized in English.
fn language_instruction(language: Option<&str>) -> String {
    match language.map(str::trim).filter(|value| !value.is_empty()) {
        Some(language) => format!("Write everything in {language}."),
        None => "Write everything in the language spoken in the transcript.".to_string(),
    }
}

fn model_file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Splits the transcript on sentence boundaries into slices that fit a map pass.
fn split_into_chunks(model: &LlamaModel, text: &str) -> Vec<String> {
    let budget = CHUNK_TOKENS;
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_tokens = 0usize;
    for sentence in sentences(text) {
        let tokens = token_count(model, sentence);
        if current_tokens > 0 && current_tokens + tokens > budget {
            chunks.push(std::mem::take(&mut current));
            current_tokens = 0;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(sentence);
        current_tokens += tokens;
    }
    if !current.trim().is_empty() {
        chunks.push(current);
    }
    chunks
}

fn token_count(model: &LlamaModel, text: &str) -> usize {
    model
        .str_to_token(text, AddBos::Never)
        .map(|tokens| tokens.len())
        // A tokenizer failure must not stop a summary; four characters per token
        // is a safe overestimate for the languages we see.
        .unwrap_or_else(|_| text.len() / 4 + 1)
}

fn sentences(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(byte, b'.' | b'!' | b'?' | b'\n')
            && bytes
                .get(index + 1)
                .is_none_or(|next| next.is_ascii_whitespace())
        {
            let part = text[start..=index].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = index + 1;
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

#[allow(clippy::too_many_arguments)]
fn generate(
    model: &LlamaModel,
    backend: &LlamaBackend,
    template: &LlamaChatTemplate,
    system: &str,
    user: &str,
    max_tokens: usize,
    threads: i32,
    mut on_progress: impl FnMut(f32),
) -> EngineResult<String> {
    let messages = vec![chat_message("system", system)?, chat_message("user", user)?];
    // The chat template supplies whatever opening token the model expects, so
    // adding another BOS here would corrupt the prompt.
    let prompt = model
        .apply_chat_template(template, &messages, true)
        .map_err(|error| format!("could not build the summary prompt: {error}"))?;
    let tokens = model
        .str_to_token(&prompt, AddBos::Never)
        .map_err(|error| format!("could not tokenize the summary prompt: {error}"))?;
    if tokens.len() + max_tokens >= CONTEXT_TOKENS as usize {
        return Err("this transcript slice does not fit the summary model's context".to_string());
    }

    let params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(CONTEXT_TOKENS))
        .with_n_batch(BATCH_TOKENS as u32)
        .with_n_ubatch(BATCH_TOKENS as u32)
        .with_n_threads(threads)
        .with_n_threads_batch(threads);
    let mut context = model
        .new_context(backend, params)
        .map_err(|error| format!("could not start the summary model: {error}"))?;

    let mut batch = LlamaBatch::new(BATCH_TOKENS, 1);
    let mut position = 0i32;
    for slice in tokens.chunks(BATCH_TOKENS) {
        batch.clear();
        for (offset, token) in slice.iter().enumerate() {
            let index = position as usize + offset;
            batch
                .add(
                    *token,
                    position + offset as i32,
                    &[0],
                    index + 1 == tokens.len(),
                )
                .map_err(|error| format!("summary prompt was rejected: {error}"))?;
        }
        context
            .decode(&mut batch)
            .map_err(|error| format!("the summary model failed: {error}"))?;
        position += slice.len() as i32;
    }

    // Low temperature keeps a summary close to the transcript, and the
    // repetition penalty stops small models looping on a bullet they like.
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::penalties(128, 1.1, 0.0, 0.0),
        LlamaSampler::temp(0.3),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::dist(0),
    ]);

    let mut output = Vec::new();
    for generated in 0..max_tokens {
        let token = sampler.sample(&context, batch.n_tokens() - 1);
        if model.is_eog_token(token) {
            break;
        }
        sampler.accept(token);
        output.extend_from_slice(&token_bytes(model, token)?);
        if generated % 16 == 0 {
            on_progress(generated as f32 / max_tokens as f32);
        }
        batch.clear();
        batch
            .add(token, position, &[0], true)
            .map_err(|error| format!("summary generation stalled: {error}"))?;
        position += 1;
        context
            .decode(&mut batch)
            .map_err(|error| format!("the summary model failed: {error}"))?;
    }
    on_progress(1.0);
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn chat_message(role: &str, content: &str) -> EngineResult<LlamaChatMessage> {
    LlamaChatMessage::new(role.to_string(), content.to_string())
        .map_err(|error| format!("could not build the summary prompt: {error}"))
}

/// Accumulating raw bytes rather than per-token strings keeps multi-byte
/// characters intact when a tokenizer splits one across two tokens.
fn token_bytes(model: &LlamaModel, token: llama_cpp_2::token::LlamaToken) -> EngineResult<Vec<u8>> {
    model
        .token_to_piece_bytes(token, 32, false, None)
        .or_else(|_| model.token_to_piece_bytes(token, 512, false, None))
        .map_err(|error| format!("the summary model returned invalid text: {error}"))
}

/// Reads the model's answer back into a [`Summary`].
///
/// Small models drift: they bold the headings, translate them, number the
/// bullets, or skip a section entirely. Parsing leniently and keeping whatever
/// survives beats failing the whole run over a stray asterisk.
fn parse_summary(answer: &str) -> Summary {
    let mut summary = Summary {
        suggested_title: String::new(),
        overview: String::new(),
        key_points: Vec::new(),
        decisions: Vec::new(),
        action_items: Vec::new(),
        model: String::new(),
        created_at: String::new(),
    };
    let mut section = Section::None;
    for line in answer.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((heading, rest)) = heading(line) {
            section = heading;
            let rest = rest.trim();
            if rest.is_empty() {
                continue;
            }
            push(&mut summary, section, rest);
            continue;
        }
        push(&mut summary, section, strip_bullet(line));
    }

    summary.suggested_title = clean_title(&summary.suggested_title);
    if summary.suggested_title.is_empty() {
        summary.suggested_title = clean_title(first_sentence(&summary.overview));
    }
    summary
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Title,
    Overview,
    KeyPoints,
    Decisions,
    ActionItems,
}

fn heading(line: &str) -> Option<(Section, &str)> {
    let bare = line
        .trim_start_matches(['#', '*', '-', '•', ' '])
        .trim_end_matches(['*', ' ']);
    let (label, rest) = bare.split_once(':')?;
    let label = label.trim().trim_matches('*').trim().to_ascii_lowercase();
    let section = match label.as_str() {
        "title" => Section::Title,
        "overview" | "summary" => Section::Overview,
        "key points" | "key point" | "keypoints" => Section::KeyPoints,
        "decisions" | "decision" => Section::Decisions,
        "action items" | "action item" | "actions" | "next steps" => Section::ActionItems,
        _ => return None,
    };
    Some((section, rest))
}

fn push(summary: &mut Summary, section: Section, text: &str) {
    let text = text.trim().trim_matches('*').trim();
    if text.is_empty() {
        return;
    }
    match section {
        Section::None => {}
        Section::Title => {
            if summary.suggested_title.is_empty() {
                summary.suggested_title = text.to_string();
            }
        }
        Section::Overview => {
            if !summary.overview.is_empty() {
                summary.overview.push(' ');
            }
            summary.overview.push_str(text);
        }
        // "None" is what the prompt asks for when a section had nothing in it.
        Section::KeyPoints if !is_none(text) => summary.key_points.push(text.into()),
        Section::Decisions if !is_none(text) => summary.decisions.push(text.into()),
        Section::ActionItems if !is_none(text) => summary.action_items.push(text.into()),
        _ => {}
    }
}

fn is_none(text: &str) -> bool {
    matches!(
        text.trim_end_matches('.')
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "none" | "n/a" | "none." | "ingen" | "keine" | "aucun"
    )
}

fn strip_bullet(line: &str) -> &str {
    let trimmed = line.trim_start_matches(['-', '*', '•', ' ']);
    // Numbered bullets: "1." or "2)".
    let digits = trimmed
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(trimmed.len());
    if digits > 0 && trimmed[digits..].starts_with(['.', ')']) {
        return trimmed[digits + 1..].trim();
    }
    trimmed.trim()
}

fn first_sentence(text: &str) -> &str {
    sentences(text).first().copied().unwrap_or("")
}

/// Titles go straight into the library list, so they lose the quoting, trailing
/// punctuation, and markdown that small models like to add.
fn clean_title(title: &str) -> String {
    let cleaned = title
        .trim()
        .trim_matches(['"', '\'', '*', '#', ' '])
        .trim_end_matches(['.', ':', ' '])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    // The library caps titles at 160 characters; leave room rather than risk a
    // rename the repository will reject.
    match cleaned.char_indices().nth(120) {
        Some((index, _)) => cleaned[..index].trim_end().to_string(),
        None => cleaned,
    }
}
fn main() {
    let mut raw = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut raw) {
        emit(&Event::Error {
            message: format!("could not read the request: {error}"),
        });
        std::process::exit(1);
    }
    let request: Request = match serde_json::from_str(&raw) {
        Ok(request) => request,
        Err(error) => {
            emit(&Event::Error {
                message: format!("could not parse the request: {error}"),
            });
            std::process::exit(1);
        }
    };

    let mut on_progress = |stage: SummarizationStage, progress: f32| {
        emit(&Event::Progress {
            stage,
            progress: progress.clamp(0.0, 1.0),
        });
    };
    match summarize(
        &request.transcript,
        Path::new(&request.model_path),
        request.summary_prompt.as_deref(),
        &mut on_progress,
    ) {
        Ok(summary) => emit(&Event::Summary(Box::new(summary))),
        Err(message) => {
            emit(&Event::Error { message });
            std::process::exit(1);
        }
    }
}

/// Every event is one line, flushed immediately: the app reads these as they
/// arrive to drive the progress bar, so buffering them would defeat the point.
fn emit(event: &Event) {
    let mut stdout = std::io::stdout();
    if let Ok(line) = serde_json::to_string(event) {
        let _ = writeln!(stdout, "{line}");
        let _ = stdout.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_layout_the_prompt_asks_for() {
        let summary = parse_summary(
            "TITLE: Pricing rollout for the Nordic launch\n\
             OVERVIEW: The team reviewed pricing tiers. They agreed to launch in March.\n\
             KEY POINTS:\n- Tier three is too close to tier two\n- Support load is the open risk\n\
             DECISIONS:\n- Launch in March\n\
             ACTION ITEMS:\n- Mette rewrites the pricing page\n",
        );

        assert_eq!(
            summary.suggested_title,
            "Pricing rollout for the Nordic launch"
        );
        assert!(
            summary
                .overview
                .starts_with("The team reviewed pricing tiers.")
        );
        assert_eq!(summary.key_points.len(), 2);
        assert_eq!(summary.decisions, vec!["Launch in March"]);
        assert_eq!(
            summary.action_items,
            vec!["Mette rewrites the pricing page"]
        );
    }

    #[test]
    fn survives_the_markdown_and_numbering_small_models_add() {
        let summary = parse_summary(
            "**TITLE:** \"Weekly standup\"\n\n\
             **Overview:**\nShort sync on open bugs.\n\n\
             ### Key Points:\n1. Login bug is still open\n2) Release moved to Friday\n\n\
             **Decisions:**\nNone\n\n\
             Action Items:\n* Ask Jonas to retest\n",
        );

        assert_eq!(summary.suggested_title, "Weekly standup");
        assert_eq!(summary.overview, "Short sync on open bugs.");
        assert_eq!(
            summary.key_points,
            vec!["Login bug is still open", "Release moved to Friday"]
        );
        assert!(summary.decisions.is_empty());
        assert_eq!(summary.action_items, vec!["Ask Jonas to retest"]);
    }

    #[test]
    fn falls_back_to_the_overview_when_no_title_was_written() {
        let summary = parse_summary("OVERVIEW: Budget review for Q3. It ran long.");

        assert_eq!(summary.suggested_title, "Budget review for Q3");
    }

    #[test]
    fn titles_stay_within_what_the_library_accepts() {
        let summary = parse_summary(&format!("TITLE: {}", "word ".repeat(60)));

        assert!(summary.suggested_title.chars().count() <= 120);
        assert!(!summary.suggested_title.ends_with(' '));
    }

    #[test]
    fn sentence_split_keeps_abbreviated_decimals_together() {
        // "3.5" has no space after the period, so it is not a boundary.
        let parts = sentences("We shipped 3.5 today. Then we stopped.");

        assert_eq!(parts, vec!["We shipped 3.5 today.", "Then we stopped."]);
    }

    #[test]
    fn language_steering_follows_the_transcript() {
        assert_eq!(
            language_instruction(Some("danish")),
            "Write everything in danish."
        );
        assert!(language_instruction(None).contains("language spoken in the transcript"));
        assert!(language_instruction(Some("  ")).contains("language spoken in the transcript"));
    }

    #[test]
    fn custom_instructions_reach_short_and_long_meeting_passes() {
        let custom = "Focus on risks and unanswered questions. Use a direct tone.";
        let notes = notes_prompt(
            0,
            2,
            "Write everything in English.",
            "A transcript slice.",
            Some(custom),
        );
        let summary = summary_prompt(
            "Write everything in English.",
            "Meeting material.",
            true,
            Some(custom),
        );

        assert!(notes.contains(custom));
        assert!(summary.contains(custom));
        assert!(summary.contains("TITLE:"));
        assert!(summary.contains("ACTION ITEMS:"));
    }

    #[test]
    fn blank_custom_instructions_use_the_default_prompt() {
        let prompt = summary_prompt(
            "Write everything in English.",
            "Meeting material.",
            false,
            Some("  "),
        );

        assert!(prompt.contains("Summarize the meeting clearly and concisely."));
    }
}
