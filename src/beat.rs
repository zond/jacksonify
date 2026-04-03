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
    let peaks = pick_peaks(&onset_signal);
    estimate_tempo(&peaks, sample_rate)
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

fn estimate_tempo(peaks: &[usize], sample_rate: f32) -> f64 {
    if peaks.len() < 2 {
        return 0.0;
    }

    let seconds_per_frame = HOP_SIZE as f64 / sample_rate as f64;
    let mut bpm_votes: Vec<f64> = Vec::new();

    for i in 1..peaks.len() {
        let ioi = (peaks[i] - peaks[i - 1]) as f64 * seconds_per_frame;
        if ioi <= 0.0 {
            continue;
        }
        let bpm = 60.0 / ioi;

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

    let mut histogram = vec![0u32; 141];

    for &bpm in &bpm_votes {
        let idx = (bpm.round() as i32 - 60).clamp(0, 140) as usize;
        histogram[idx] += 1;
        if idx > 0 {
            histogram[idx - 1] += 1;
        }
        if idx < 140 {
            histogram[idx + 1] += 1;
        }
    }

    let best_idx = histogram
        .iter()
        .enumerate()
        .max_by_key(|(_, &count)| count)
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    (best_idx + 60) as f64
}
