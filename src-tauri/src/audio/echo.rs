use std::collections::VecDeque;

use sonora::{
    AudioProcessing, Config, StreamConfig,
    config::{EchoCanceller, MaxProcessingRate, Pipeline},
};

use super::{FRAME_SAMPLES, SAMPLE_RATE};

// The capture queues can be skewed by up to 100 ms before filling a missing
// source with silence. Give AEC a little more reference history than that so
// it can also handle a system callback arriving after the corresponding mic
// audio. Delay BOTH recorded sources; this adds latency, not recording length.
const LOOKAHEAD_FRAMES: usize = 12;
pub(super) type Frame = [f32; FRAME_SAMPLES];

pub(super) struct CleanFrame {
    pub microphone: Frame,
    pub system: Frame,
}

pub(super) struct EchoProcessor {
    processor: AudioProcessing,
    pending: VecDeque<CleanFrame>,
    failed: bool,
    warning_pending: bool,
}

impl EchoProcessor {
    pub fn new() -> Self {
        let stream = StreamConfig::new(SAMPLE_RATE, 1);
        Self {
            processor: AudioProcessing::builder()
                .config(Config {
                    pipeline: Pipeline {
                        maximum_internal_processing_rate: MaxProcessingRate::Rate48kHz,
                        ..Default::default()
                    },
                    echo_canceller: Some(EchoCanceller::default()),
                    ..Default::default()
                })
                .capture_config(stream)
                .render_config(stream)
                .build(),
            pending: VecDeque::with_capacity(LOOKAHEAD_FRAMES + 1),
            failed: false,
            warning_pending: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn take_warning(&mut self) -> bool {
        std::mem::take(&mut self.warning_pending)
    }

    fn fail(&mut self) {
        if !self.failed {
            self.warning_pending = true;
            self.failed = true;
        }
    }

    fn render(&mut self, system: &Frame) {
        let mut scratch = [0.0; FRAME_SAMPLES];
        if !self.failed
            && self
                .processor
                .process_render_f32(&[system], &mut [&mut scratch])
                .is_err()
        {
            self.fail();
        }
    }

    pub fn push(&mut self, microphone: Frame, system: Frame) -> Option<CleanFrame> {
        self.render(&system);
        self.pending.push_back(CleanFrame { microphone, system });
        if self.pending.len() > LOOKAHEAD_FRAMES {
            self.capture()
        } else {
            None
        }
    }

    fn capture(&mut self) -> Option<CleanFrame> {
        let mut frame = self.pending.pop_front()?;
        let mut cleaned = [0.0; FRAME_SAMPLES];
        if !self.failed {
            if self
                .processor
                .process_capture_f32(&[&frame.microphone], &mut [&mut cleaned])
                .is_ok()
            {
                frame.microphone = cleaned;
            } else {
                // Preserve the original microphone if processing ever fails.
                // The worker surfaces a warning, rather than silently muting it.
                self.fail();
            }
        }
        Some(frame)
    }

    // Flush on stop, pause and source changes. Advance reference time with
    // silence without appending that silence to the recording.
    pub fn flush(&mut self) -> Option<CleanFrame> {
        if self.pending.is_empty() {
            return None;
        }
        self.render(&[0.0; FRAME_SAMPLES]);
        self.capture()
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    // Deterministic, speech-band excitation. Never use user recordings in tests.
    pub fn signal(samples: usize, mut seed: u32) -> Vec<f32> {
        let mut low = 0.0;
        let mut dc = 0.0;
        (0..samples)
            .map(|i| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
                low += 0.3 * (noise - low);
                dc += 0.02 * (low - dc);
                let envelope = 0.6 + 0.4 * (i as f32 / 8000.0).sin();
                (low - dc) * 0.4 * envelope
            })
            .collect()
    }

    pub fn echo(system: &[f32], delay: isize) -> Vec<f32> {
        let at = |i: isize| {
            usize::try_from(i)
                .ok()
                .and_then(|i| system.get(i))
                .copied()
                .unwrap_or(0.0)
        };
        (0..system.len())
            .map(|i| {
                let i = i as isize - delay;
                at(i) * 0.55 + at(i - 431) * 0.2 - at(i - 1037) * 0.1
            })
            .collect()
    }

    pub fn energy(signal: &[f32]) -> f32 {
        signal.iter().map(|x| x * x).sum::<f32>() / signal.len() as f32
    }

    fn process(microphone: &[f32], system: &[f32]) -> Vec<f32> {
        let mut processor = EchoProcessor::new();
        let mut clean = Vec::new();
        let mut reference = Vec::new();
        for (mic, sys) in microphone
            .chunks_exact(FRAME_SAMPLES)
            .zip(system.chunks_exact(FRAME_SAMPLES))
        {
            if let Some(frame) = processor.push(mic.try_into().unwrap(), sys.try_into().unwrap()) {
                clean.extend(frame.microphone);
                reference.extend(frame.system);
            }
        }
        while let Some(frame) = processor.flush() {
            clean.extend(frame.microphone);
            reference.extend(frame.system);
        }
        assert!(!processor.take_warning());
        assert_eq!(reference, system, "computer audio must remain unchanged");
        assert_eq!(
            clean.len(),
            microphone.len(),
            "lookahead must not change duration"
        );
        assert!(clean.iter().all(|x| x.is_finite()));
        clean
    }

    #[test]
    fn cancels_delayed_echo_and_room_reflections() {
        let system = signal(SAMPLE_RATE as usize * 10, 0x12345678);
        // Negative delay models a system callback arriving after mic audio.
        for delay_ms in [-60, 40, 180] {
            let microphone = echo(&system, delay_ms * 48);
            let clean = process(&microphone, &system);
            let tail = SAMPLE_RATE as usize * 7;
            let ratio = energy(&clean[tail..]) / energy(&microphone[tail..]);
            assert!(ratio < 0.03, "delay {delay_ms} ms: residual energy {ratio}");
        }
    }

    #[test]
    fn preserves_overlapping_local_speech() {
        let count = SAMPLE_RATE as usize * 12;
        let system = signal(count, 0x12345678);
        // Voiced speech-like harmonics with a changing pitch and independent
        // syllable envelope; broadband noise is intentionally used for the
        // far-end excitation, not as a stand-in for local speech.
        let mut phase = 0.0_f32;
        let mut local: Vec<f32> = (0..count)
            .map(|i| {
                let time = i as f32 / SAMPLE_RATE as f32;
                phase += std::f32::consts::TAU * (170.0 + 30.0 * (time * 2.3).sin())
                    / SAMPLE_RATE as f32;
                let envelope = (time * 9.0).sin().max(0.0) * 0.8 + 0.2;
                (1..=12)
                    .map(|harmonic| (phase * harmonic as f32).sin() / harmonic as f32)
                    .sum::<f32>()
                    * 0.15
                    * envelope
            })
            .collect();
        local[..SAMPLE_RATE as usize * 5].fill(0.0);
        let microphone: Vec<_> = echo(&system, 60 * 48)
            .iter()
            .zip(&local)
            .map(|(a, b)| a + b)
            .collect();
        let clean = process(&microphone, &system);
        let baseline = process(&local, &vec![0.0; count]);
        // Compare with the same voice through the same filter bank, with no
        // speaker reference. AEC's high-pass/analysis filters change phase as
        // well as delay, so raw sample correlation is not a speech metric.
        let start = SAMPLE_RATE as usize * 8;
        let expected = &baseline[start..];
        let actual = &clean[start..];
        let expected_energy = energy(expected);
        let actual_energy = energy(actual);
        let cross =
            actual.iter().zip(expected).map(|(a, b)| a * b).sum::<f32>() / actual.len() as f32;
        let correlation = cross / (actual_energy * expected_energy).sqrt();
        let gain = cross / expected_energy;
        assert!(
            expected_energy > energy(&local[start..]) * 0.65,
            "voice must remain audible with a silent reference (headphones)"
        );
        assert!(
            correlation > 0.85,
            "local speech correlation: {correlation}"
        );
        assert!((0.65..1.25).contains(&gain), "local speech gain: {gain}");
    }

    #[test]
    fn flushes_short_recordings_and_reset_drops_old_reference() {
        let mut processor = EchoProcessor::new();
        for _ in 0..3 {
            assert!(
                processor
                    .push([0.0; FRAME_SAMPLES], [0.25; FRAME_SAMPLES])
                    .is_none()
            );
        }
        assert_eq!(std::iter::from_fn(|| processor.flush()).count(), 3);
        processor.push([0.1; FRAME_SAMPLES], [0.25; FRAME_SAMPLES]);
        processor.reset();
        assert!(processor.flush().is_none());
        for _ in 0..20 {
            if let Some(frame) = processor.push([0.0; FRAME_SAMPLES], [0.0; FRAME_SAMPLES]) {
                assert_eq!(frame.system, [0.0; FRAME_SAMPLES]);
                assert!(frame.microphone.iter().all(|x| x.abs() < 1e-6));
            }
        }
        assert_eq!(
            std::iter::from_fn(|| processor.flush()).count(),
            LOOKAHEAD_FRAMES
        );
    }

    #[test]
    fn processing_failure_preserves_audio_and_reports_once() {
        let mut processor = EchoProcessor::new();
        processor.fail();
        assert!(processor.take_warning());
        processor.fail();
        assert!(!processor.take_warning());
        let microphone = [0.1; FRAME_SAMPLES];
        let system = [0.2; FRAME_SAMPLES];
        processor.push(microphone, system);
        let frame = processor.flush().unwrap();
        assert_eq!(frame.microphone, microphone);
        assert_eq!(frame.system, system);
    }
}
