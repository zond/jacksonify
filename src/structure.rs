use crate::beat::BeatGrid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    Intro,
    Verse,
    Chorus,
    Bridge,
    Outro,
}

pub struct Section {
    pub section_type: SectionType,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub occurrence: u32,
}

pub fn analyze_structure(
    samples: &[f32],
    sample_rate: f32,
    beat_grid: &BeatGrid,
) -> Vec<Section> {
    if beat_grid.beats.is_empty() {
        let dur = samples.len() as f64 / sample_rate as f64;
        return vec![Section {
            section_type: SectionType::Chorus,
            start_seconds: 0.0,
            end_seconds: dur,
            occurrence: 1,
        }];
    }

    // Compute energy per bar
    let energy_envelope = compute_bar_energy(samples, sample_rate, beat_grid);

    if energy_envelope.is_empty() {
        let dur = samples.len() as f64 / sample_rate as f64;
        return vec![Section {
            section_type: SectionType::Chorus,
            start_seconds: 0.0,
            end_seconds: dur,
            occurrence: 1,
        }];
    }

    // Detect section boundaries using energy changes
    let boundaries = detect_boundaries(&energy_envelope);

    // Label sections
    label_sections(&energy_envelope, &boundaries, beat_grid, samples.len() as f64 / sample_rate as f64)
}

fn compute_bar_energy(
    samples: &[f32],
    sample_rate: f32,
    beat_grid: &BeatGrid,
) -> Vec<f64> {
    // Group beats by bar and compute RMS for each bar
    let mut bars: Vec<(f64, f64)> = Vec::new(); // (start, end) times

    let mut current_bar = 0u32;
    let mut bar_start = 0.0;

    for beat in &beat_grid.beats {
        if beat.bar_number != current_bar {
            if bar_start > 0.0 || current_bar == 0 {
                bars.push((bar_start, beat.time_seconds));
            }
            current_bar = beat.bar_number;
            bar_start = beat.time_seconds;
        }
    }
    // Last bar
    if let Some(last) = beat_grid.beats.last() {
        bars.push((bar_start, last.time_seconds + beat_grid.beat_period_secs));
    }

    // Compute RMS energy for each bar
    bars.iter()
        .map(|&(start, end)| {
            let s = (start * sample_rate as f64) as usize;
            let e = ((end * sample_rate as f64) as usize).min(samples.len());
            if s >= e || s >= samples.len() {
                return 0.0;
            }
            let sum_sq: f64 = samples[s..e]
                .iter()
                .map(|&x| (x as f64) * (x as f64))
                .sum();
            (sum_sq / (e - s) as f64).sqrt()
        })
        .collect()
}

fn detect_boundaries(energy: &[f64]) -> Vec<usize> {
    if energy.len() < 8 {
        return vec![];
    }

    // Smooth energy with a 4-bar window
    let smooth_window = 4;
    let smoothed: Vec<f64> = energy
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let start = i.saturating_sub(smooth_window / 2);
            let end = (i + smooth_window / 2 + 1).min(energy.len());
            energy[start..end].iter().sum::<f64>() / (end - start) as f64
        })
        .collect();

    // Detect significant energy changes (boundaries)
    // Use a difference signal: |smoothed[i+4] - smoothed[i]|
    let look_ahead = 4;
    let mut diff = Vec::new();
    for i in 0..smoothed.len().saturating_sub(look_ahead) {
        let d = (smoothed[i + look_ahead] - smoothed[i]).abs();
        diff.push(d);
    }

    if diff.is_empty() {
        return vec![];
    }

    // Find peaks in diff signal (section boundaries)
    let mean: f64 = diff.iter().sum::<f64>() / diff.len() as f64;
    let std: f64 = (diff.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / diff.len() as f64).sqrt();
    let threshold = mean + 0.5 * std;

    let mut boundaries = Vec::new();
    let min_section_bars = 6; // Minimum 6 bars per section

    for i in 1..diff.len() - 1 {
        if diff[i] > diff[i - 1] && diff[i] > diff.get(i + 1).copied().unwrap_or(0.0)
            && diff[i] > threshold
        {
            // Enforce minimum section length
            if boundaries.is_empty() || i - boundaries.last().unwrap() >= min_section_bars {
                boundaries.push(i + look_ahead / 2); // Center the boundary
            }
        }
    }

    boundaries
}

fn label_sections(
    energy: &[f64],
    boundaries: &[usize],
    beat_grid: &BeatGrid,
    duration: f64,
) -> Vec<Section> {
    // Create section time ranges from boundaries
    let mut section_ranges: Vec<(usize, usize)> = Vec::new();
    let mut prev = 0;
    for &b in boundaries {
        let b = b.min(energy.len());
        if b > prev {
            section_ranges.push((prev, b));
        }
        prev = b;
    }
    if prev < energy.len() {
        section_ranges.push((prev, energy.len()));
    }

    if section_ranges.is_empty() {
        return vec![Section {
            section_type: SectionType::Chorus,
            start_seconds: 0.0,
            end_seconds: duration,
            occurrence: 1,
        }];
    }

    // Compute average energy per section
    let section_energies: Vec<f64> = section_ranges
        .iter()
        .map(|&(s, e)| {
            if s >= e || s >= energy.len() {
                return 0.0;
            }
            let end = e.min(energy.len());
            energy[s..end].iter().sum::<f64>() / (end - s) as f64
        })
        .collect();

    // Find energy thresholds for classification
    let max_energy = section_energies.iter().cloned().fold(0.0f64, f64::max);
    let mean_energy = section_energies.iter().sum::<f64>() / section_energies.len() as f64;

    // Helper: bar index to time
    let bar_to_time = |bar: usize| -> f64 {
        beat_grid
            .beats
            .iter()
            .find(|b| b.bar_number as usize == bar)
            .map(|b| b.time_seconds)
            .unwrap_or(bar as f64 * beat_grid.beat_period_secs * 4.0)
    };

    // Label sections
    let mut sections: Vec<Section> = Vec::new();
    let mut verse_count = 0u32;
    let mut chorus_count = 0u32;

    for (idx, &(start_bar, end_bar)) in section_ranges.iter().enumerate() {
        let e = section_energies[idx];
        let start_time = bar_to_time(start_bar);
        let end_time = if end_bar >= energy.len() {
            duration
        } else {
            bar_to_time(end_bar)
        };

        let is_first = idx == 0;
        let is_last = idx == section_ranges.len() - 1;

        let section_type = if is_first && e < mean_energy * 0.7 && (end_time - start_time) < 20.0 {
            SectionType::Intro
        } else if is_last && (end_time - start_time) > 10.0 {
            SectionType::Outro
        } else if e > mean_energy * 1.1 || e > max_energy * 0.85 {
            chorus_count += 1;
            SectionType::Chorus
        } else if e < mean_energy * 0.9 {
            verse_count += 1;
            SectionType::Verse
        } else {
            // Ambiguous — if we already have verses and choruses, call it a bridge
            if verse_count > 0 && chorus_count > 0 {
                SectionType::Bridge
            } else {
                verse_count += 1;
                SectionType::Verse
            }
        };

        let occurrence = match section_type {
            SectionType::Chorus => chorus_count,
            SectionType::Verse => verse_count,
            _ => 1,
        };

        sections.push(Section {
            section_type,
            start_seconds: start_time,
            end_seconds: end_time,
            occurrence,
        });
    }

    if sections.is_empty() {
        sections.push(Section {
            section_type: SectionType::Chorus,
            start_seconds: 0.0,
            end_seconds: duration,
            occurrence: 1,
        });
    }

    sections
}
