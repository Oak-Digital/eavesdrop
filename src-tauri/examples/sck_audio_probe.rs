//! Diagnostic: does ScreenCaptureKit actually deliver system audio right now?
//!
//! Mirrors the exact filter/config Eavesdrop uses in `platform::macos`, then
//! reports what arrives: buffer count, channel counts, measured sample rate and
//! RMS level. Run it while audio is playing, once per output device, to tell a
//! "SCK sends nothing" fault apart from a "SCK sends the wrong format" one.
//!
//!     cargo run --example sck_audio_probe

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("this probe is macOS-only");
}

#[cfg(target_os = "macos")]
fn default_output_device() -> String {
    let out = std::process::Command::new("system_profiler")
        .arg("SPAudioDataType")
        .output();
    let Ok(out) = out else {
        return "unknown".into();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut current = "unknown".to_string();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with(':') && !trimmed.contains("Device") && !trimmed.is_empty() {
            current = trimmed.trim_end_matches(':').to_string();
        }
        if trimmed.starts_with("Default Output Device: Yes") {
            return current;
        }
    }
    "unknown".into()
}

#[cfg(target_os = "macos")]
fn main() {
    use screencapturekit::prelude::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::{Duration, Instant};

    let content = SCShareableContent::get().expect("screen recording permission");
    let display = content
        .displays()
        .into_iter()
        .next()
        .expect("at least one display");
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_applications(&[], &[])
        .build();
    let config = SCStreamConfiguration::new()
        .with_width(2)
        .with_height(2)
        .with_captures_audio(true)
        .with_excludes_current_process_audio(true)
        .with_sample_rate(48_000)
        .with_channel_count(1);

    let buffers_seen = Arc::new(AtomicU64::new(0));
    let samples_seen = Arc::new(AtomicU64::new(0));
    let channels_seen = Arc::new(AtomicU64::new(0));
    let sub_buffers_seen = Arc::new(AtomicU64::new(0));
    // RMS is accumulated as a fixed-point sum of squares so it fits in an atomic.
    let energy = Arc::new(AtomicU64::new(0));

    let mut stream = SCStream::new(&filter, &config);
    {
        let (buffers, samples, channels, subs, energy) = (
            buffers_seen.clone(),
            samples_seen.clone(),
            channels_seen.clone(),
            sub_buffers_seen.clone(),
            energy.clone(),
        );
        stream.add_output_handler(
            move |sample: CMSampleBuffer, output_type| {
                if output_type != SCStreamOutputType::Audio {
                    return;
                }
                buffers.fetch_add(1, Ordering::Relaxed);
                let Some(list) = sample.audio_buffer_list() else {
                    return;
                };
                for buffer in list.iter() {
                    subs.fetch_add(1, Ordering::Relaxed);
                    channels.fetch_add(buffer.number_channels.max(1) as u64, Ordering::Relaxed);
                    let pcm: Vec<f32> = buffer
                        .data()
                        .chunks_exact(4)
                        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
                        .collect();
                    samples.fetch_add(pcm.len() as u64, Ordering::Relaxed);
                    let sum: f64 = pcm.iter().map(|s| (*s as f64) * (*s as f64)).sum();
                    energy.fetch_add((sum * 1_000_000.0) as u64, Ordering::Relaxed);
                }
            },
            SCStreamOutputType::Audio,
        );
    }

    stream.start_capture().expect("start capture");
    println!("default output at start : {}", default_output_device());
    println!(
        "\ncapturing for 40s. Keep audio playing the whole time.\n\
         To test a device switch, change the output device (e.g. connect AirPods)\n\
         around t=10s and watch whether samples keep arriving.\n"
    );
    let started = Instant::now();
    let mut last = (0u64, 0u64);
    let mut silent_seconds = 0u32;
    let mut stalled_after: Option<u32> = None;
    for second in 1..=40 {
        std::thread::sleep(Duration::from_secs(1));
        let b = buffers_seen.load(Ordering::Relaxed);
        let s = samples_seen.load(Ordering::Relaxed);
        let e = energy.load(Ordering::Relaxed);
        let delta_samples = s - last.1;
        let rms = if s > 0 {
            ((e as f64 / 1_000_000.0) / s as f64).sqrt()
        } else {
            0.0
        };
        if delta_samples == 0 {
            silent_seconds += 1;
            if stalled_after.is_none() && s > 0 {
                stalled_after = Some(second);
            }
        }
        println!(
            "t={second:2}s  buffers={b:5} (+{:4})  samples={s:8} (+{delta_samples:6}/s)  rms={rms:.5}{}",
            b - last.0,
            if delta_samples == 0 { "   <-- no samples this second" } else { "" }
        );
        last = (b, s);
    }
    let _ = stream.stop_capture();

    let elapsed = started.elapsed().as_secs_f64();
    let samples = samples_seen.load(Ordering::Relaxed);
    let subs = sub_buffers_seen.load(Ordering::Relaxed);
    let channels = channels_seen.load(Ordering::Relaxed);
    let e = energy.load(Ordering::Relaxed);
    println!("\n--- summary ---");
    println!("sample buffers      : {}", buffers_seen.load(Ordering::Relaxed));
    println!("audio sub-buffers   : {subs}");
    println!(
        "avg channels/buffer : {:.2}",
        if subs > 0 { channels as f64 / subs as f64 } else { 0.0 }
    );
    println!("total f32 samples   : {samples}");
    println!(
        "measured rate       : {:.0} Hz  (app assumes 48000)",
        samples as f64 / elapsed
    );
    println!("default output at end: {}", default_output_device());
    println!("seconds with 0 samples: {silent_seconds}");
    if let Some(t) = stalled_after {
        println!("STREAM STALLED after t={t}s — audio was flowing, then stopped");
    }
    println!(
        "overall rms         : {:.5}{}",
        if samples > 0 { ((e as f64 / 1_000_000.0) / samples as f64).sqrt() } else { 0.0 },
        if samples == 0 { "   <-- NO AUDIO DELIVERED" } else { "" }
    );
}
