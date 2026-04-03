use crate::beat::BeatGrid;
use crate::gaps::VocalGap;
use crate::key::KeySegment;
use crate::structure::{Section, SectionType};

#[derive(Clone)]
pub struct AdLibPlacement {
    pub time_seconds: f64,
    pub sample_file: String,
    pub pitch_shift_semitones: i8,
    pub playback_rate: f32, // 1.0 = normal speed, <1 = slower, >1 = faster
    pub gain: f32,
    pub priority: f32, // 0.0-1.0, higher = placed at lower slider values
}

struct SampleInfo {
    file: &'static str,
    category: Category,
    pitched: bool,
    estimated_pitch_class: Option<u8>, // 0-11
}

#[derive(Clone, Copy, PartialEq)]
enum Category {
    HeeHee,
    Shout,
    Grunt,
    Moan,
    Shamone,
    VocalPercussion,
    Exclamation,
}

struct Candidate {
    time: f64,
    categories: Vec<Category>,
    beat_strength: f32,
    is_gap_fill: bool, // true = placed in a gap rather than on a beat
}

fn sample_catalog() -> Vec<SampleInfo> {
    vec![
        SampleInfo { file: "hee-hee.mp3", category: Category::HeeHee, pitched: true, estimated_pitch_class: Some(8) }, // G#4
        SampleInfo { file: "eeeehe.mp3", category: Category::HeeHee, pitched: true, estimated_pitch_class: Some(4) }, // E5
        SampleInfo { file: "eeeh.mp3", category: Category::HeeHee, pitched: true, estimated_pitch_class: Some(3) }, // D#5
        SampleInfo { file: "eeh.mp3", category: Category::HeeHee, pitched: true, estimated_pitch_class: Some(5) }, // F5
        SampleInfo { file: "yow.mp3", category: Category::Shout, pitched: true, estimated_pitch_class: Some(6) }, // F#5
        SampleInfo { file: "ieow.mp3", category: Category::Shout, pitched: true, estimated_pitch_class: Some(11) }, // B4
        SampleInfo { file: "gadaaao.mp3", category: Category::Shout, pitched: true, estimated_pitch_class: Some(10) }, // A#4
        SampleInfo { file: "ah.mp3", category: Category::Grunt, pitched: true, estimated_pitch_class: Some(9) }, // A3
        SampleInfo { file: "aeh.mp3", category: Category::Grunt, pitched: true, estimated_pitch_class: Some(1) }, // C#5
        SampleInfo { file: "eh.mp3", category: Category::Grunt, pitched: true, estimated_pitch_class: Some(4) }, // E5
        SampleInfo { file: "uh-duh.mp3", category: Category::Grunt, pitched: true, estimated_pitch_class: Some(6) }, // F#4
        SampleInfo { file: "uh-uh.mp3", category: Category::Grunt, pitched: true, estimated_pitch_class: Some(4) }, // E5
        SampleInfo { file: "aaah.mp3", category: Category::Exclamation, pitched: true, estimated_pitch_class: Some(8) }, // G#4
        SampleInfo { file: "ooooh.mp3", category: Category::Exclamation, pitched: true, estimated_pitch_class: Some(10) }, // A#4
        SampleInfo { file: "yeeeah.mp3", category: Category::Exclamation, pitched: true, estimated_pitch_class: Some(4) }, // E5
        SampleInfo { file: "yeeahaaa.mp3", category: Category::Exclamation, pitched: true, estimated_pitch_class: Some(6) }, // F#4
        SampleInfo { file: "haaaaoo.mp3", category: Category::Moan, pitched: true, estimated_pitch_class: Some(11) }, // B4
        SampleInfo { file: "ooooou-hooooou.mp3", category: Category::Moan, pitched: true, estimated_pitch_class: Some(9) }, // A4
        SampleInfo { file: "ooouuh.mp3", category: Category::Moan, pitched: true, estimated_pitch_class: Some(11) }, // B4
        SampleInfo { file: "ouh-hou.mp3", category: Category::Moan, pitched: true, estimated_pitch_class: Some(7) }, // G5
        SampleInfo { file: "uooou.mp3", category: Category::Moan, pitched: true, estimated_pitch_class: Some(1) }, // C#4
        SampleInfo { file: "shamone.mp3", category: Category::Shamone, pitched: true, estimated_pitch_class: Some(8) }, // G#4
        SampleInfo { file: "shaka.mp3", category: Category::VocalPercussion, pitched: false, estimated_pitch_class: None },
        SampleInfo { file: "shaka-shaka.mp3", category: Category::VocalPercussion, pitched: false, estimated_pitch_class: None },
        SampleInfo { file: "dah-dah-dah.mp3", category: Category::VocalPercussion, pitched: false, estimated_pitch_class: None },
    ]
}

pub fn place_ad_libs(
    beat_grid: &BeatGrid,
    key_segments: &[KeySegment],
    sections: &[Section],
    gaps: &[VocalGap],
) -> Vec<AdLibPlacement> {
    if beat_grid.beats.is_empty() {
        return vec![];
    }

    let catalog = sample_catalog();
    let tempo = beat_grid.tempo_bpm;
    let mut candidates = generate_beat_candidates(beat_grid, tempo);
    let mut gap_candidates = generate_gap_candidates(gaps, beat_grid);
    candidates.append(&mut gap_candidates);
    candidates.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

    let candidates = filter_by_gaps(candidates, gaps);
    assign_and_prioritize(candidates, sections, key_segments, &catalog, tempo)
}

fn generate_beat_candidates(beat_grid: &BeatGrid, tempo: f64) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let is_ballad = tempo < 90.0;
    let is_uptempo = tempo > 110.0;

    for (idx, beat) in beat_grid.beats.iter().enumerate() {
        // Downbeats (1, 3): shouts and exclamations
        // At ballad tempos, prefer exclamations over shouts
        if beat.beat_in_bar == 1 || beat.beat_in_bar == 3 {
            let cats = if is_ballad {
                vec![Category::Exclamation]
            } else if beat.beat_in_bar == 1 {
                // Beat 1: allow shamone (especially at section starts)
                vec![Category::Shout, Category::Exclamation, Category::Shamone]
            } else {
                vec![Category::Shout, Category::Exclamation]
            };
            candidates.push(Candidate {
                time: beat.time_seconds,
                categories: cats,
                beat_strength: if beat.beat_in_bar == 1 { 0.9 } else { 0.7 },
                is_gap_fill: false,
            });
        }

        // Backbeats (2, 4): grunts and vocal percussion
        // At ballad tempos, skip percussive sounds
        if (beat.beat_in_bar == 2 || beat.beat_in_bar == 4) && !is_ballad {
            candidates.push(Candidate {
                time: beat.time_seconds,
                categories: vec![Category::Grunt, Category::VocalPercussion],
                beat_strength: 0.6,
                is_gap_fill: false,
            });
        }

        // Upbeats of 2 and 4: hee-hee (functions like hi-hat accent)
        if beat.beat_in_bar == 2 || beat.beat_in_bar == 4 {
            let cats = if is_ballad {
                vec![Category::Exclamation] // Ballads: melodic ad-libs on upbeats
            } else {
                vec![Category::HeeHee, Category::Exclamation]
            };
            candidates.push(Candidate {
                time: beat_grid.upbeat_after(idx),
                categories: cats,
                beat_strength: 0.5,
                is_gap_fill: false,
            });
        }

        // Upbeats of 1 and 3
        if beat.beat_in_bar == 1 || beat.beat_in_bar == 3 {
            candidates.push(Candidate {
                time: beat_grid.upbeat_after(idx),
                categories: vec![Category::HeeHee, Category::Exclamation],
                beat_strength: 0.3,
                is_gap_fill: false,
            });
        }

        // 16th notes at uptempo
        if is_uptempo {
            candidates.push(Candidate {
                time: beat_grid.sixteenth_e(idx),
                categories: vec![Category::HeeHee, Category::Grunt],
                beat_strength: 0.15,
                is_gap_fill: false,
            });
        }
    }

    candidates
}

/// Generate candidates for moans/wails/melodic phrases in vocal gaps.
/// These fill the spaces between vocal phrases rather than sitting on beat positions.
fn generate_gap_candidates(gaps: &[VocalGap], beat_grid: &BeatGrid) -> Vec<Candidate> {
    let mut candidates = Vec::new();

    for gap in gaps {
        let gap_duration = gap.end_seconds - gap.start_seconds;
        if gap_duration < 0.4 {
            continue; // Too short for a moan/phrase
        }

        // Place a moan/exclamation in the center of sufficiently long gaps
        let center = (gap.start_seconds + gap.end_seconds) / 2.0;
        let cats = if gap_duration > 1.0 {
            vec![Category::Moan, Category::Exclamation, Category::Shamone]
        } else {
            vec![Category::Moan, Category::Exclamation]
        };

        // Snap to nearest beat subdivision for rhythmic coherence
        let snapped = snap_to_grid(center, beat_grid);

        candidates.push(Candidate {
            time: snapped,
            categories: cats,
            beat_strength: 0.45,
            is_gap_fill: true,
        });
    }

    candidates
}

fn snap_to_grid(time: f64, beat_grid: &BeatGrid) -> f64 {
    let mut best = time;
    let mut best_dist = f64::MAX;
    for beat in &beat_grid.beats {
        // Check beat, upbeat, and 16th positions
        for &t in &[
            beat.time_seconds,
            beat.time_seconds + 0.5 * beat_grid.beat_period_secs,
            beat.time_seconds + 0.25 * beat_grid.beat_period_secs,
        ] {
            let dist = (t - time).abs();
            if dist < best_dist {
                best_dist = dist;
                best = t;
            }
        }
    }
    best
}

fn filter_by_gaps(candidates: Vec<Candidate>, gaps: &[VocalGap]) -> Vec<Candidate> {
    if gaps.is_empty() {
        return candidates;
    }

    let tolerance = 0.05;

    candidates
        .into_iter()
        .filter(|c| {
            // Gap-fill candidates are already in gaps by construction
            if c.is_gap_fill {
                return true;
            }
            gaps.iter().any(|g| {
                let is_percussive = c.categories.iter().any(|cat| {
                    matches!(cat, Category::Grunt | Category::VocalPercussion)
                });
                let tol = if is_percussive { tolerance } else { 0.0 };
                c.time >= g.start_seconds - tol && c.time <= g.end_seconds + tol
            })
        })
        .collect()
}

fn assign_and_prioritize(
    candidates: Vec<Candidate>,
    sections: &[Section],
    key_segments: &[KeySegment],
    catalog: &[SampleInfo],
    tempo: f64,
) -> Vec<AdLibPlacement> {
    let mut placements = Vec::new();

    let mut last_category: Option<Category> = None;
    let mut last_file: Option<&str> = None;
    let mut last_time = f64::NEG_INFINITY;
    let mut pick_counter: usize = 0;

    // Tempo-based playback rate adjustment for groove feel
    let groove_rate = if tempo < 85.0 {
        0.92 // Slow down samples for laid-back ballad feel
    } else if tempo < 100.0 {
        0.96
    } else if tempo > 140.0 {
        1.06 // Speed up slightly for snappy uptempo feel
    } else {
        1.0
    };

    for candidate in &candidates {
        let section = sections
            .iter()
            .find(|s| candidate.time >= s.start_seconds && candidate.time < s.end_seconds);

        let (density_mult, section_type) = match section {
            Some(s) => {
                let base = match s.section_type {
                    SectionType::Intro => 0.05,
                    SectionType::Verse => 0.15,
                    SectionType::Chorus => 0.4,
                    SectionType::Bridge => 0.25,
                    SectionType::Outro => 0.6,
                };
                let escalation = 1.0 + 0.2 * (s.occurrence.saturating_sub(1) as f64);
                (base * escalation, s.section_type)
            }
            None => (0.3, SectionType::Chorus),
        };

        let tempo_mult = if tempo < 90.0 {
            0.3
        } else if tempo > 130.0 {
            1.5
        } else {
            1.0
        };

        let available: Vec<&SampleInfo> = catalog
            .iter()
            .filter(|s| candidate.categories.contains(&s.category))
            .collect();

        if available.is_empty() {
            continue;
        }

        let min_spacing = if tempo > 110.0 { 0.25 } else { 0.5 };
        if candidate.time - last_time < min_spacing {
            continue;
        }

        // Pick sample — rotate, avoid repeats
        let sample = {
            let mut choice = available
                .iter()
                .filter(|s| {
                    Some(s.file) != last_file
                        && last_category.map_or(true, |lc| s.category != lc)
                })
                .nth(pick_counter % available.len().max(1));

            if choice.is_none() {
                choice = available
                    .iter()
                    .filter(|s| Some(s.file) != last_file)
                    .nth(pick_counter % available.len().max(1));
            }

            choice.unwrap_or(&available[pick_counter % available.len()])
        };
        pick_counter += 1;

        let priority = (density_mult * tempo_mult * candidate.beat_strength as f64)
            .clamp(0.0, 1.0) as f32;

        // Pitch shift: target depends on category (per MJ analysis doc)
        let pitch_shift = compute_pitch_shift(sample, candidate.time, key_segments, pick_counter);

        // Playback rate: groove adjustment + pitch shift component
        // The pitch_shift changes pitch via playbackRate in JS, so we combine
        // the groove rate with any additional speed feel
        let playback_rate = groove_rate;

        let gain = match section_type {
            SectionType::Intro => 0.3,
            SectionType::Verse => 0.5,
            SectionType::Chorus => 0.7,
            SectionType::Bridge => 0.6,
            SectionType::Outro => 0.8,
        } as f32;

        // Micro-timing swing for live feel
        let swing = if tempo < 120.0 {
            let phase = (candidate.time * 17.3).sin();
            0.010 + 0.020 * (phase * 0.5 + 0.5)
        } else {
            0.0
        };

        placements.push(AdLibPlacement {
            time_seconds: candidate.time + swing,
            sample_file: sample.file.to_string(),
            pitch_shift_semitones: pitch_shift,
            playback_rate: playback_rate as f32,
            gain,
            priority,
        });

        last_category = Some(sample.category);
        last_file = Some(sample.file);
        last_time = candidate.time;
    }

    placements.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap());
    placements
}

/// Compute pitch shift based on category-specific target intervals.
/// From the MJ vocal analysis:
/// - HeeHee: root or 5th
/// - Shout/Ow: root or b7 (bluesy feel)
/// - Moan/Exclamation: root, 3rd, or 5th
/// - Shamone: root
fn compute_pitch_shift(
    sample: &SampleInfo,
    time: f64,
    key_segments: &[KeySegment],
    variation: usize,
) -> i8 {
    if !sample.pitched {
        return 0;
    }
    let sample_pc = match sample.estimated_pitch_class {
        Some(pc) => pc,
        None => return 0,
    };

    let key_seg = key_segments
        .iter()
        .find(|k| time >= k.start_seconds && time < k.end_seconds)
        .or(key_segments.first());

    let ks = match key_seg {
        Some(k) => k,
        None => return 0,
    };

    let root = ks.root_pitch_class;
    let third = (root + 4) % 12;      // major 3rd (works for both, close enough)
    let fifth = (root + 7) % 12;
    let flat_seven = (root + 10) % 12; // b7

    // Pick target pitch class based on category and variation for variety
    let targets = match sample.category {
        Category::HeeHee => vec![root, fifth],
        Category::Shout => vec![root, flat_seven],
        Category::Shamone => vec![root],
        Category::Grunt => return 0, // Grunts are percussive, don't shift much
        Category::VocalPercussion => return 0,
        Category::Moan | Category::Exclamation => vec![root, third, fifth],
    };

    let target = targets[variation % targets.len()];

    let shift = ((target as i8 - sample_pc as i8) + 12) % 12;
    let shift = if shift > 6 { shift - 12 } else { shift };

    // Constrain to +/- 5 semitones
    if shift.abs() <= 5 { shift } else { 0 }
}
