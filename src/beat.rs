use rustfft::{num_complex::Complex, FftPlanner};

const FRAME_SIZE: usize = 1024;
const HOP_SIZE: usize = 512;

pub fn detect_tempo(samples: &[f32], sample_rate: f32) -> f64 {
    if samples.len() < FRAME_SIZE {
        return 0.0;
    }

    let onset_signal = compute_spectral_flux(samples);
    let peaks = pick_peaks(&onset_signal);
    estimate_tempo(&peaks, sample_rate)
}

fn compute_spectral_flux(samples: &[f32]) -> Vec<f32> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FRAME_SIZE);

    let num_frames = (samples.len() - FRAME_SIZE) / HOP_SIZE + 1;
    let mut flux = Vec::with_capacity(num_frames);
    let mut prev_magnitude = vec![0.0f32; FRAME_SIZE / 2 + 1];

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
                let sample = samples[start + j];
                Complex::new(sample * window[j], 0.0)
            })
            .collect();

        fft.process(&mut buffer);

        // Half-wave rectified spectral flux (only count increases)
        let mut frame_flux = 0.0f32;
        for j in 0..=FRAME_SIZE / 2 {
            let mag = buffer[j].norm();
            let diff = mag - prev_magnitude[j];
            if diff > 0.0 {
                frame_flux += diff;
            }
            prev_magnitude[j] = mag;
        }

        flux.push(frame_flux);
    }

    flux
}

fn pick_peaks(signal: &[f32]) -> Vec<usize> {
    if signal.len() < 3 {
        return vec![];
    }

    let window = 10;
    let factor = 0.5;
    let mut peaks = Vec::new();

    for i in 1..signal.len() - 1 {
        let start = i.saturating_sub(window);
        let end = (i + window + 1).min(signal.len());
        let local_slice = &signal[start..end];

        let mean = local_slice.iter().sum::<f32>() / local_slice.len() as f32;
        let std = (local_slice
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f32>()
            / local_slice.len() as f32)
            .sqrt();
        let threshold = mean + factor * std;

        if signal[i] > signal[i - 1] && signal[i] > signal[i + 1] && signal[i] > threshold {
            peaks.push(i);
        }
    }

    peaks
}

fn estimate_tempo(peaks: &[usize], sample_rate: f32) -> f64 {
    if peaks.len() < 2 {
        return 0.0;
    }

    let seconds_per_frame = HOP_SIZE as f64 / sample_rate as f64;

    // Collect BPM values from inter-onset intervals
    let mut bpm_votes: Vec<f64> = Vec::new();

    for i in 1..peaks.len() {
        let ioi = (peaks[i] - peaks[i - 1]) as f64 * seconds_per_frame;
        if ioi <= 0.0 {
            continue;
        }
        let bpm = 60.0 / ioi;

        // Fold into 60-200 BPM range (double/halve as needed)
        let mut folded = bpm;
        while folded < 60.0 {
            folded *= 2.0;
        }
        while folded > 200.0 {
            folded /= 2.0;
        }
        if folded >= 60.0 && folded <= 200.0 {
            bpm_votes.push(folded);
        }
    }

    if bpm_votes.is_empty() {
        return 0.0;
    }

    // Build histogram with 1 BPM resolution
    let mut histogram = vec![0u32; 141]; // indices 0..140 => BPM 60..200

    for &bpm in &bpm_votes {
        let idx = (bpm.round() as i32 - 60).clamp(0, 140) as usize;
        histogram[idx] += 1;
        // Also vote for neighbors for smoothing
        if idx > 0 {
            histogram[idx - 1] += 1;
        }
        if idx < 140 {
            histogram[idx + 1] += 1;
        }
    }

    // Find peak in histogram
    let best_idx = histogram
        .iter()
        .enumerate()
        .max_by_key(|(_, &count)| count)
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    (best_idx + 60) as f64
}
