//! Benchmark: how long does Whisper take on this machine, and does the GPU
//! backend actually engage?
//!
//! Mirrors the production parameters in `transcription::transcribe` exactly, so
//! the timings here are the ones users feel. Feed it 16 kHz mono signed 16-bit
//! WAV, which is Whisper's native input format:
//!
//!     say -f script.txt -o bench.wav --data-format=LEI16@16000
//!     cargo run --release --example whisper_bench -- ggml-base.bin bench.wav
//!
//! Build it with and without `--features whisper-rs/metal` to compare backends.
//! ggml prints the backend it selected to stderr as the model loads.

use std::path::{Path, PathBuf};
use std::time::Instant;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn models_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME is set");
    PathBuf::from(home).join("Library/Application Support/com.eavesdrop.recorder/models")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let model = match args.next() {
        Some(value) if value.contains('/') => PathBuf::from(value),
        Some(value) => models_dir().join(value),
        None => models_dir().join("ggml-base.bin"),
    };
    let wav = PathBuf::from(args.next().unwrap_or_else(|| "bench.wav".into()));

    let pcm = read_wav_16k_mono(&wav).unwrap_or_else(|error| {
        eprintln!("could not read {}: {error}", wav.display());
        std::process::exit(1);
    });
    let audio_seconds = pcm.len() as f32 / 16_000.0;
    println!("model: {}", model.display());
    println!("audio: {:.1}s ({} samples)\n", audio_seconds, pcm.len());

    let load_started = Instant::now();
    let context = WhisperContext::new_with_params(&model, WhisperContextParameters::default())
        .unwrap_or_else(|error| {
            eprintln!("could not load model: {error}");
            std::process::exit(1);
        });
    let load_elapsed = load_started.elapsed();
    let mut state = context.create_state().expect("could not create state");

    // Identical to transcription::transcribe, so these numbers transfer.
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 2 });
    let threads = std::thread::available_parallelism()
        .map(|count| count.get().saturating_sub(1).clamp(1, 8))
        .unwrap_or(2);
    params.set_n_threads(threads as i32);
    params.set_translate(false);
    params.set_language(None);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    let started = Instant::now();
    state.full(params, &pcm).expect("transcription failed");
    let elapsed = started.elapsed();

    let show_text = std::env::var_os("BENCH_TEXT").is_some();
    let mut words = 0usize;
    let mut text = String::new();
    for segment in state.as_iter() {
        let Ok(segment) = segment.to_str_lossy() else {
            continue;
        };
        words += segment.split_whitespace().count();
        if show_text {
            text.push_str(segment.trim());
            text.push(' ');
        }
    }
    if show_text {
        println!("\n--- transcript ---\n{}", text.trim());
    }

    println!("\nthreads:      {threads}");
    println!("model load:   {:.2}s", load_elapsed.as_secs_f32());
    println!("transcribe:   {:.2}s", elapsed.as_secs_f32());
    println!(
        "realtime:     {:.1}x",
        audio_seconds / elapsed.as_secs_f32()
    );
    println!("words out:    {words}");
}

/// Minimal WAV reader. Walks the RIFF chunk list rather than assuming a 44-byte
/// header, because `say` writes a LIST chunk before the data.
fn read_wav_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut cursor = 12;
    let mut format = None;
    while cursor + 8 <= bytes.len() {
        let id = &bytes[cursor..cursor + 4];
        let size = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let body = cursor + 8;
        let end = (body + size).min(bytes.len());
        match id {
            b"fmt " if size >= 16 => {
                let channels = u16::from_le_bytes(bytes[body + 2..body + 4].try_into().unwrap());
                let rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().unwrap());
                let bits = u16::from_le_bytes(bytes[body + 14..body + 16].try_into().unwrap());
                if channels != 1 || rate != 16_000 || bits != 16 {
                    return Err(format!(
                        "expected 1ch/16000Hz/16-bit, got {channels}ch/{rate}Hz/{bits}-bit"
                    ));
                }
                format = Some(());
            }
            b"data" => {
                if format.is_none() {
                    return Err("data chunk before fmt chunk".into());
                }
                return Ok(bytes[body..end]
                    .chunks_exact(2)
                    .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / i16::MAX as f32)
                    .collect());
            }
            _ => {}
        }
        cursor = body + size + (size & 1);
    }
    Err("no data chunk".into())
}
