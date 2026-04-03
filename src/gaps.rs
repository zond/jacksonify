use rustfft::{num_complex::Complex, FftPlanner};

const FRAME_SIZE: usize = 1024;
const HOP_SIZE: usize = 256;

pub struct VocalGap {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

pub fn detect_vocal_gaps(samples: &[f32], sample_rate: f32) -> Vec<VocalGap> {
    if samples.len() < FRAME_SIZE {
        return vec![];
    }

    let vocal_energy = compute_vocal_band_energy(samples, sample_rate);
    let total_energy = compute_total_energy(samples);

    if vocal_energy.is_empty() || total_energy.is_empty() {
        return vec![];
    }

    let seconds_per_frame = HOP_SIZE as f64 / sample_rate as f64;

    // Compute vocal ratio: vocal_band / total
    let len = vocal_energy.len().min(total_energy.len());
    let ratio: Vec<f64> = (0..len)
        .map(|i| {
            if total_energy[i] > 1e-10 {
                vocal_energy[i] / total_energy[i]
            } else {
                0.0
            }
        })
        .collect();

    // Smooth with a running median approximation (use running mean for speed)
    let smooth_window = (0.2 / seconds_per_frame) as usize; // ~200ms
    let smooth_window = smooth_window.max(1);
    let smoothed: Vec<f64> = ratio
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let start = i.saturating_sub(smooth_window / 2);
            let end = (i + smooth_window / 2 + 1).min(ratio.len());
            ratio[start..end].iter().sum::<f64>() / (end - start) as f64
        })
        .collect();

    // Adaptive threshold: below median indicates a gap
    let mut sorted = smoothed.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let threshold = median * 0.6;

    // Find gap regions
    let min_gap_frames = (0.2 / seconds_per_frame) as usize; // 200ms minimum
    let max_gap_frames = (3.0 / seconds_per_frame) as usize; // 3s maximum

    let mut gaps = Vec::new();
    let mut gap_start: Option<usize> = None;

    for (i, &v) in smoothed.iter().enumerate() {
        if v < threshold {
            if gap_start.is_none() {
                gap_start = Some(i);
            }
        } else if let Some(start) = gap_start {
            let length = i - start;
            if length >= min_gap_frames && length <= max_gap_frames {
                gaps.push(VocalGap {
                    start_seconds: start as f64 * seconds_per_frame,
                    end_seconds: i as f64 * seconds_per_frame,
                });
            }
            gap_start = None;
        }
    }

    // Handle gap at end of audio
    if let Some(start) = gap_start {
        let length = smoothed.len() - start;
        if length >= min_gap_frames {
            gaps.push(VocalGap {
                start_seconds: start as f64 * seconds_per_frame,
                end_seconds: smoothed.len() as f64 * seconds_per_frame,
            });
        }
    }

    gaps
}

fn compute_vocal_band_energy(samples: &[f32], sample_rate: f32) -> Vec<f64> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FRAME_SIZE);

    let num_frames = (samples.len() - FRAME_SIZE) / HOP_SIZE + 1;
    let mut energy = Vec::with_capacity(num_frames);

    // Vocal band: 300-3000 Hz
    let low_bin = (300.0 * FRAME_SIZE as f32 / sample_rate).round() as usize;
    let high_bin = (3000.0 * FRAME_SIZE as f32 / sample_rate).round() as usize;
    let high_bin = high_bin.min(FRAME_SIZE / 2);

    let window: Vec<f32> = (0..FRAME_SIZE)
        .map(|j| {
            0.5 * (1.0
                - (2.0 * std::f32::consts::PI * j as f32 / (FRAME_SIZE - 1) as f32).cos())
        })
        .collect();

    for i in 0..num_frames {
        let start = i * HOP_SIZE;
        if start + FRAME_SIZE > samples.len() {
            break;
        }

        let mut buffer: Vec<Complex<f32>> = (0..FRAME_SIZE)
            .map(|j| Complex::new(samples[start + j] * window[j], 0.0))
            .collect();

        fft.process(&mut buffer);

        let vocal_e: f64 = (low_bin..=high_bin)
            .map(|b| {
                let m = buffer[b].norm() as f64;
                m * m
            })
            .sum();

        energy.push(vocal_e);
    }

    energy
}

fn compute_total_energy(samples: &[f32]) -> Vec<f64> {
    let num_frames = (samples.len() - FRAME_SIZE) / HOP_SIZE + 1;
    let mut energy = Vec::with_capacity(num_frames);

    for i in 0..num_frames {
        let start = i * HOP_SIZE;
        let end = (start + FRAME_SIZE).min(samples.len());
        let e: f64 = samples[start..end]
            .iter()
            .map(|&x| (x as f64) * (x as f64))
            .sum();
        energy.push(e);
    }

    energy
}
