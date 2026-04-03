use rustfft::{num_complex::Complex, FftPlanner};

const FRAME_SIZE: usize = 1024;
const HOP_SIZE: usize = 512;

pub struct BeatGrid {
    pub tempo_bpm: f64,
    pub beat_period_secs: f64,
    pub beats: Vec<BeatInfo>,
}

pub struct BeatInfo {
    pub time_seconds: f64,
    pub beat_in_bar: u8, // 1, 2, 3, or 4
    pub bar_number: u32,
}

impl BeatGrid {
    pub fn upbeat_after(&self, idx: usize) -> f64 {
        self.beats[idx].time_seconds + 0.5 * self.beat_period_secs
    }

    pub fn sixteenth_e(&self, idx: usize) -> f64 {
        self.beats[idx].time_seconds + 0.25 * self.beat_period_secs
    }

}

pub fn detect_tempo(samples: &[f32], sample_rate: f32) -> f64 {
    if samples.len() < FRAME_SIZE {
        return 0.0;
    }
    let onset_signal = compute_spectral_flux(samples);
    estimate_tempo_autocorrelation(&onset_signal, sample_rate)
}

pub fn detect_beat_grid(samples: &[f32], sample_rate: f32) -> BeatGrid {
    let tempo = detect_tempo(samples, sample_rate);
    if tempo <= 0.0 {
        return BeatGrid {
            tempo_bpm: 0.0,
            beat_period_secs: 0.0,
            beats: vec![],
        };
    }

    let beat_period = 60.0 / tempo;
    let duration = samples.len() as f64 / sample_rate as f64;
    let onset_signal = compute_spectral_flux(samples);
    let peaks = pick_peaks(&onset_signal);
    let seconds_per_frame = HOP_SIZE as f64 / sample_rate as f64;

    // Detect phase using onset peaks
    let phase = detect_phase(&onset_signal, &peaks, tempo, seconds_per_frame);

    // Generate beat grid
    let num_beats = ((duration - phase) / beat_period) as usize;
    let mut beat_times: Vec<f64> = (0..num_beats)
        .map(|i| phase + i as f64 * beat_period)
        .collect();

    // Snap each beat to nearest onset peak within tolerance
    let tolerance = beat_period * 0.15;
    for bt in &mut beat_times {
        let mut best_dist = tolerance;
        let mut best_peak_time = *bt;
        for &p in &peaks {
            let pt = p as f64 * seconds_per_frame;
            let dist = (pt - *bt).abs();
            if dist < best_dist {
                best_dist = dist;
                best_peak_time = pt;
            }
        }
        *bt = best_peak_time;
    }

    // Downbeat estimation
    let downbeat_offset = estimate_downbeat_offset(samples, sample_rate, &beat_times);

    let beats: Vec<BeatInfo> = beat_times
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let beat_in_bar = ((i + downbeat_offset) % 4) as u8 + 1;
            let bar_number = ((i + downbeat_offset) / 4) as u32;
            BeatInfo {
                time_seconds: t,
                beat_in_bar,
                bar_number,
            }
        })
        .collect();

    BeatGrid {
        tempo_bpm: tempo,
        beat_period_secs: beat_period,
        beats,
    }
}

fn detect_phase(
    onset_signal: &[f32],
    peaks: &[usize],
    tempo: f64,
    seconds_per_frame: f64,
) -> f64 {
    if peaks.is_empty() {
        return 0.0;
    }

    let beat_period_frames = 60.0 / (tempo * seconds_per_frame);
    let period = beat_period_frames.round() as usize;
    if period == 0 || period > onset_signal.len() {
        return 0.0;
    }

    // Accumulate onset energy at each phase offset
    let mut phase_energy = vec![0.0f64; period];
    for &p in peaks {
        let phase_bin = p % period;
        phase_energy[phase_bin] += onset_signal[p] as f64;
    }

    let best_phase = phase_energy
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);

    best_phase as f64 * seconds_per_frame
}

fn estimate_downbeat_offset(
    samples: &[f32],
    sample_rate: f32,
    beat_times: &[f64],
) -> usize {
    if beat_times.len() < 8 {
        return 0;
    }

    let window_samples = (0.05 * sample_rate as f64) as usize; // 50ms window

    // Compute bass and mid energy at each beat
    let mut bass_energy: Vec<f64> = Vec::new();
    let mut mid_energy: Vec<f64> = Vec::new();

    for &t in beat_times {
        let center = (t * sample_rate as f64) as usize;
        let start = center.saturating_sub(window_samples / 2);
        let end = (center + window_samples / 2).min(samples.len());
        if start >= end {
            bass_energy.push(0.0);
            mid_energy.push(0.0);
            continue;
        }

        let chunk: Vec<f32> = samples[start..end].to_vec();
        // Simple energy split: bass = low-pass approximation, mid = remainder
        // Use running average as crude low-pass
        let lp_size = (sample_rate as usize / 150).max(2); // ~150Hz cutoff
        let mut bass = 0.0f64;
        let mut total = 0.0f64;
        for (i, &s) in chunk.iter().enumerate() {
            total += (s as f64) * (s as f64);
            if i >= lp_size {
                let avg: f32 = chunk[i - lp_size..=i].iter().sum::<f32>() / lp_size as f32;
                bass += (avg as f64) * (avg as f64);
            }
        }
        bass_energy.push(bass);
        mid_energy.push(total - bass);
    }

    // Try each rotation (0-3), score by how well beats 1,3 have more bass
    // and beats 2,4 have more mid energy
    let mut best_offset = 0;
    let mut best_score = f64::NEG_INFINITY;

    for offset in 0..4 {
        let mut score = 0.0;
        for i in 0..beat_times.len().min(bass_energy.len()) {
            let beat_in_bar = (i + offset) % 4;
            match beat_in_bar {
                0 | 2 => score += bass_energy[i], // beats 1,3 should have bass
                1 | 3 => score += mid_energy[i],  // beats 2,4 should have snare/mid
                _ => {}
            }
        }
        if score > best_score {
            best_score = score;
            best_offset = offset;
        }
    }

    best_offset
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

/// Estimate tempo via autocorrelation of the onset signal.
/// This finds the dominant periodicity directly, which is much more robust
/// than IOI histograms from peak picking.
fn estimate_tempo_autocorrelation(onset_signal: &[f32], sample_rate: f32) -> f64 {
    if onset_signal.len() < 100 {
        return 0.0;
    }

    let seconds_per_frame = HOP_SIZE as f64 / sample_rate as f64;

    // Lag range corresponding to 40-220 BPM
    let min_lag = (60.0 / (220.0 * seconds_per_frame)) as usize; // fastest tempo
    let max_lag = (60.0 / (40.0 * seconds_per_frame)) as usize;  // slowest tempo
    let max_lag = max_lag.min(onset_signal.len() / 2);

    if min_lag >= max_lag {
        return 0.0;
    }

    // Compute mean for normalization
    let mean: f64 = onset_signal.iter().map(|&x| x as f64).sum::<f64>() / onset_signal.len() as f64;

    // Compute autocorrelation for each lag
    let n = onset_signal.len();
    let mut best_lag = min_lag;
    let mut best_corr = f64::NEG_INFINITY;

    // Precompute zero-mean signal
    let centered: Vec<f64> = onset_signal.iter().map(|&x| x as f64 - mean).collect();

    // Normalization: variance at lag 0
    let var: f64 = centered.iter().map(|x| x * x).sum::<f64>();
    if var < 1e-10 {
        return 0.0;
    }

    let mut autocorr = vec![0.0f64; max_lag + 1];
    for lag in min_lag..=max_lag {
        let mut sum = 0.0;
        for i in 0..n - lag {
            sum += centered[i] * centered[i + lag];
        }
        autocorr[lag] = sum / var;
    }

    // Apply perceptual tempo weighting — Gaussian centered at 120 BPM
    // This biases toward common musical tempos and prevents spurious
    // peaks at extreme BPM values from dominating.
    let mut weighted = vec![0.0f64; max_lag + 1];
    for lag in min_lag..=max_lag {
        let bpm = 60.0 / (lag as f64 * seconds_per_frame);
        // Gaussian weight: prefer 80-160 BPM range, centered at 120
        let w = (-(bpm - 120.0).powi(2) / (2.0 * 40.0_f64.powi(2))).exp();
        weighted[lag] = autocorr[lag] * w;
    }

    // Find the best weighted peak
    for lag in min_lag..=max_lag {
        if weighted[lag] > best_corr {
            best_corr = weighted[lag];
            best_lag = lag;
        }
    }

    // Refine: check if there's a peak at half the lag (double tempo) that's
    // also strong — if so, prefer the faster tempo to avoid half-tempo errors
    let half_lag = best_lag / 2;
    if half_lag >= min_lag {
        let half_bpm = 60.0 / (half_lag as f64 * seconds_per_frame);
        if half_bpm >= 60.0 && half_bpm <= 200.0 && autocorr[half_lag] > autocorr[best_lag] * 0.6 {
            best_lag = half_lag;
        }
    }

    let bpm = 60.0 / (best_lag as f64 * seconds_per_frame);

    // Fold into 60-200 BPM range
    let mut folded = bpm;
    while folded < 60.0 {
        folded *= 2.0;
    }
    while folded > 200.0 {
        folded /= 2.0;
    }

    // Round to nearest 0.5 BPM
    (folded * 2.0).round() / 2.0
}
