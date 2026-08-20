//! Diagnostic: does a local summary actually come out, and can whisper.cpp and
//! llama.cpp share one process?
//!
//! Both engines statically link their own copy of ggml, so the linker resolves
//! them to a single implementation. This probe runs Whisper and then the
//! summary model back to back, which is the only way to prove that merge is
//! benign at runtime rather than only at link time.
//!
//!     cargo run --example summary_probe -- [transcript.txt]
//!
//! Without an argument it summarizes a short built-in transcript. Models are
//! read from the installed app's models directory.

use std::path::PathBuf;

use eavesdrop_lib::probe::{Summary, SummarizationStage, Transcript, summarize};

const SAMPLE: &str = "\
Right, so the main thing today is the pricing page. Mette walked us through the three tiers. \
The problem is tier three sits too close to tier two, only forty kroner apart, so nobody has a \
reason to move up. Jonas said support load is the real risk if we push everyone onto tier one. \
We went back and forth on whether to delay, but we agreed to launch in March anyway and revisit \
the tier three price after the first month. Mette is rewriting the pricing page copy this week. \
Jonas will pull the support ticket numbers from last quarter before Friday so we have a baseline.";

fn models_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME is set");
    PathBuf::from(home).join("Library/Application Support/com.eavesdrop.recorder/models")
}

fn main() {
    let models = models_dir();

    // Whisper first: it loads its ggml, and anything the shared-symbol merge
    // broke shows up here before llama.cpp has run at all.
    let whisper_model = models.join("ggml-base.bin");
    if whisper_model.is_file() {
        print!("whisper.cpp: ");
        match run_whisper(&whisper_model) {
            Ok(()) => println!("ok"),
            Err(error) => {
                println!("FAILED — {error}");
                std::process::exit(1);
            }
        }
    } else {
        println!("whisper.cpp: skipped, no model at {}", whisper_model.display());
    }

    let summary_model = std::fs::read_dir(&models)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "gguf"));
    let Some(summary_model) = summary_model else {
        eprintln!("no .gguf summary model in {}", models.display());
        std::process::exit(1);
    };
    println!("summary model: {}", summary_model.display());

    let text = match std::env::args().nth(1) {
        Some(path) => std::fs::read_to_string(&path).expect("could not read the transcript"),
        None => SAMPLE.to_string(),
    };
    let transcript = Transcript {
        text,
        language: Some("english".into()),
        created_at: String::new(),
        segments: Vec::new(),
    };

    let started = std::time::Instant::now();
    let mut last = String::new();
    let mut on_progress = |stage: SummarizationStage, progress: f32| {
        let line = format!("{stage:?} {:>3.0}%", progress * 100.0);
        if line != last {
            println!("  {line}");
            last = line;
        }
    };
    match summarize(&transcript, &summary_model, &mut on_progress) {
        Ok(summary) => {
            println!("\nfinished in {:.1}s\n", started.elapsed().as_secs_f32());
            print_summary(&summary);
        }
        Err(error) => {
            eprintln!("summary failed: {error}");
            std::process::exit(1);
        }
    }
}

fn print_summary(summary: &Summary) {
    println!("TITLE:    {}", summary.suggested_title);
    println!("OVERVIEW: {}", summary.overview);
    for (heading, points) in [
        ("KEY POINTS", &summary.key_points),
        ("DECISIONS", &summary.decisions),
        ("ACTION ITEMS", &summary.action_items),
    ] {
        println!("{heading}:");
        for point in points {
            println!("  - {point}");
        }
    }
}

/// One second of a tone is enough: we are testing that ggml still works in this
/// process, not that Whisper heard anything in particular.
fn run_whisper(model: &std::path::Path) -> Result<(), String> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    let context = WhisperContext::new_with_params(
        model.to_str().ok_or("model path is not UTF-8")?,
        WhisperContextParameters::default(),
    )
    .map_err(|error| error.to_string())?;
    let mut state = context.create_state().map_err(|error| error.to_string())?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(2);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    let pcm: Vec<f32> = (0..16_000)
        .map(|index| (index as f32 * 220.0 * std::f32::consts::TAU / 16_000.0).sin() * 0.1)
        .collect();
    state.full(params, &pcm).map_err(|error| error.to_string())?;
    Ok(())
}
