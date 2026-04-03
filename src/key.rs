use rustfft::{num_complex::Complex, FftPlanner};

const FRAME_SIZE: usize = 4096;
const HOP_SIZE: usize = 2048;

const MAJOR_PROFILE: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR_PROFILE: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub struct KeySegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub key: String,
    pub root_pitch_class: u8,
    pub confidence: f64,
}

pub fn detect_key(samples: &[f32], sample_rate: f32) -> (String, f64) {
    if samples.len() < FRAME_SIZE {
        return ("Unknown".to_string(), 0.0);
    }
    let chroma = compute_chromagram(samples, sample_rate);
    correlate_with_profiles(&chroma)
}

pub fn detect_key_segments(
    samples: &[f32],
    sample_rate: f32,
    window_sec: f64,
) -> Vec<KeySegment> {
    let window_samples = (window_sec * sample_rate as f64) as usize;
    let hop_samples = window_samples / 2;

    if samples.len() < FRAME_SIZE {
        return vec![];
    }

    let mut raw_segments: Vec<(f64, f64, String, u8, f64)> = Vec::new();
    let mut pos = 0;

    while pos + FRAME_SIZE <= samples.len() {
        let end = (pos + window_samples).min(samples.len());
        let chunk = &samples[pos..end];
        let chroma = compute_chromagram(chunk, sample_rate);

        let mut best_key = String::new();
        let mut best_corr = f64::NEG_INFINITY;
        let mut best_root = 0u8;

        for shift in 0..12 {
            let major_corr =
                pearson_correlation(&chroma, &rotate_profile(&MAJOR_PROFILE, shift));
            let minor_corr =
                pearson_correlation(&chroma, &rotate_profile(&MINOR_PROFILE, shift));

            if major_corr > best_corr {
                best_corr = major_corr;
                best_key = format!("{} major", NOTE_NAMES[shift]);
                best_root = shift as u8;
            }
            if minor_corr > best_corr {
                best_corr = minor_corr;
                best_key = format!("{} minor", NOTE_NAMES[shift]);
                best_root = shift as u8;
            }
        }

        let start_sec = pos as f64 / sample_rate as f64;
        let end_sec = end as f64 / sample_rate as f64;
        let confidence = ((best_corr + 1.0) / 2.0).clamp(0.0, 1.0);

        raw_segments.push((start_sec, end_sec, best_key, best_root, confidence));

        pos += hop_samples;
    }

    // Merge consecutive segments with the same key
    let mut merged: Vec<KeySegment> = Vec::new();
    for (start, end, key, root, conf) in raw_segments {
        if let Some(last) = merged.last_mut() {
            if last.key == key {
                last.end_seconds = end;
                last.confidence = last.confidence.max(conf);
                continue;
            }
        }
        merged.push(KeySegment {
            start_seconds: start,
            end_seconds: end,
            key,
            root_pitch_class: root,
            confidence: conf,
        });
    }

    // Remove very short segments (< 4 seconds) — assign them to surrounding key
    let min_segment_duration = 4.0;
    let mut filtered: Vec<KeySegment> = Vec::new();
    for seg in merged {
        let dur = seg.end_seconds - seg.start_seconds;
        if dur < min_segment_duration {
            if let Some(last) = filtered.last_mut() {
                last.end_seconds = seg.end_seconds;
                continue;
            }
        }
        filtered.push(seg);
    }

    if filtered.is_empty() && !samples.is_empty() {
        let (key, confidence) = detect_key(samples, sample_rate);
        filtered.push(KeySegment {
            start_seconds: 0.0,
            end_seconds: samples.len() as f64 / sample_rate as f64,
            key,
            root_pitch_class: 0,
            confidence,
        });
    }

    filtered
}

fn compute_chromagram(samples: &[f32], sample_rate: f32) -> [f64; 12] {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FRAME_SIZE);

    let num_frames = if samples.len() >= FRAME_SIZE {
        (samples.len() - FRAME_SIZE) / HOP_SIZE + 1
    } else {
        return [0.0; 12];
    };
    let mut chroma = [0.0f64; 12];

    let window: Vec<f32> = (0..FRAME_SIZE)
        .map(|j| {
            0.5 * (1.0
                - (2.0 * std::f32::consts::PI * j as f32 / (FRAME_SIZE - 1) as f32).cos())
        })
        .collect();

    for i in 0..num_frames {
        let start = i * HOP_SIZE;

        let mut buffer: Vec<Complex<f32>> = (0..FRAME_SIZE)
            .map(|j| {
                let sample = if start + j < samples.len() {
                    samples[start + j]
                } else {
                    0.0
                };
                Complex::new(sample * window[j], 0.0)
            })
            .collect();

        fft.process(&mut buffer);

        for bin in 1..=FRAME_SIZE / 2 {
            let freq = bin as f64 * sample_rate as f64 / FRAME_SIZE as f64;
            if freq < 65.0 || freq > 2000.0 {
                continue;
            }

            let midi = 69.0 + 12.0 * (freq / 440.0).log2();
            let pitch_class = ((midi.round() as i32 % 12) + 12) % 12;

            let magnitude = buffer[bin].norm() as f64;
            chroma[pitch_class as usize] += magnitude * magnitude;
        }
    }

    let max = chroma.iter().cloned().fold(0.0f64, f64::max);
    if max > 0.0 {
        for c in &mut chroma {
            *c /= max;
        }
    }

    chroma
}

fn correlate_with_profiles(chroma: &[f64; 12]) -> (String, f64) {
    let mut best_key = String::new();
    let mut best_correlation = f64::NEG_INFINITY;

    for shift in 0..12 {
        let major_corr = pearson_correlation(chroma, &rotate_profile(&MAJOR_PROFILE, shift));
        let minor_corr = pearson_correlation(chroma, &rotate_profile(&MINOR_PROFILE, shift));

        if major_corr > best_correlation {
            best_correlation = major_corr;
            best_key = format!("{} major", NOTE_NAMES[shift]);
        }
        if minor_corr > best_correlation {
            best_correlation = minor_corr;
            best_key = format!("{} minor", NOTE_NAMES[shift]);
        }
    }

    let confidence = ((best_correlation + 1.0) / 2.0).clamp(0.0, 1.0);
    (best_key, confidence)
}

fn rotate_profile(profile: &[f64; 12], shift: usize) -> [f64; 12] {
    let mut rotated = [0.0; 12];
    for i in 0..12 {
        rotated[i] = profile[(i + 12 - shift) % 12];
    }
    rotated
}

fn pearson_correlation(a: &[f64; 12], b: &[f64; 12]) -> f64 {
    let n = 12.0;
    let mean_a: f64 = a.iter().sum::<f64>() / n;
    let mean_b: f64 = b.iter().sum::<f64>() / n;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for i in 0..12 {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom < 1e-10 {
        return 0.0;
    }

    cov / denom
}
