use crate::beat::BeatGrid;
use crate::gaps::VocalGap;
use crate::key::KeySegment;
use crate::structure::{Section, SectionType};

#[derive(Clone)]
pub struct AdLibPlacement {
    pub time_seconds: f64,
    pub sample_file: String,
    pub pitch_shift_semitones: i8,
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
    beat_strength: f32, // higher = more important rhythmically
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
    let candidates = generate_candidates(beat_grid);
    let candidates = filter_by_gaps(candidates, gaps);
    assign_and_prioritize(candidates, sections, key_segments, &catalog, beat_grid.tempo_bpm)
}

fn generate_candidates(beat_grid: &BeatGrid) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let tempo = beat_grid.tempo_bpm;

    for (idx, beat) in beat_grid.beats.iter().enumerate() {
        // Downbeats (1, 3): shouts and exclamations
        if beat.beat_in_bar == 1 || beat.beat_in_bar == 3 {
            candidates.push(Candidate {
                time: beat.time_seconds,
                categories: vec![Category::Shout, Category::Exclamation, Category::Shamone],
                beat_strength: if beat.beat_in_bar == 1 { 0.9 } else { 0.7 },
            });
        }

        // Backbeats (2, 4): grunts
        if beat.beat_in_bar == 2 || beat.beat_in_bar == 4 {
            candidates.push(Candidate {
                time: beat.time_seconds,
                categories: vec![Category::Grunt, Category::VocalPercussion],
                beat_strength: 0.6,
            });
        }

        // Upbeats of 2 and 4: hee-hee
        if beat.beat_in_bar == 2 || beat.beat_in_bar == 4 {
            candidates.push(Candidate {
                time: beat_grid.upbeat_after(idx),
                categories: vec![Category::HeeHee, Category::Exclamation],
                beat_strength: 0.5,
            });
        }

        // & of 4: gasps/moans (pre-downbeat)
        if beat.beat_in_bar == 4 {
            candidates.push(Candidate {
                time: beat_grid.upbeat_after(idx),
                categories: vec![Category::Moan],
                beat_strength: 0.4,
            });
        }

        // Upbeats of 1 and 3: exclamations and hee-hee
        if beat.beat_in_bar == 1 || beat.beat_in_bar == 3 {
            candidates.push(Candidate {
                time: beat_grid.upbeat_after(idx),
                categories: vec![Category::HeeHee, Category::Exclamation],
                beat_strength: 0.3,
            });
        }

        // 16th notes at higher tempos
        if tempo > 100.0 {
            candidates.push(Candidate {
                time: beat_grid.sixteenth_e(idx),
                categories: vec![Category::HeeHee, Category::Grunt],
                beat_strength: 0.15,
            });
        }
    }

    // Sort by time
    candidates.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    candidates
}

fn filter_by_gaps(candidates: Vec<Candidate>, gaps: &[VocalGap]) -> Vec<Candidate> {
    if gaps.is_empty() {
        // If no gaps detected, keep all candidates (fallback)
        return candidates;
    }

    let tolerance = 0.05; // 50ms tolerance for percussive sounds

    candidates
        .into_iter()
        .filter(|c| {
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

    // Track recently used samples to add variety
    let mut last_category: Option<Category> = None;
    let mut last_file: Option<&str> = None;
    let mut last_time = f64::NEG_INFINITY;
    let mut pick_counter: usize = 0;

    for candidate in &candidates {
        // Find which section this candidate is in
        let section = sections
            .iter()
            .find(|s| candidate.time >= s.start_seconds && candidate.time < s.end_seconds);

        // Compute section-based density multiplier
        let (density_mult, section_type) = match section {
            Some(s) => {
                let base = match s.section_type {
                    SectionType::Intro => 0.05,
                    SectionType::Verse => 0.15,
                    SectionType::Chorus => 0.4,
                    SectionType::Bridge => 0.25,
                    SectionType::Outro => 0.6,
                };
                // Escalation: each occurrence increases density
                let escalation = 1.0 + 0.2 * (s.occurrence.saturating_sub(1) as f64);
                (base * escalation, s.section_type)
            }
            None => (0.3, SectionType::Chorus),
        };

        // Tempo-based adjustment
        let tempo_mult = if tempo < 90.0 {
            0.3
        } else if tempo > 130.0 {
            1.5
        } else {
            1.0
        };

        // Pick a sample from the candidate's categories
        let available: Vec<&SampleInfo> = catalog
            .iter()
            .filter(|s| candidate.categories.contains(&s.category))
            .collect();

        if available.is_empty() {
            continue;
        }

        // Avoid repeating the same category too quickly
        let min_spacing = if tempo > 110.0 { 0.25 } else { 0.5 };
        if candidate.time - last_time < min_spacing {
            continue;
        }

        // Pick sample — avoid repeating the same file, rotate through available
        let sample = {
            // First try: different file AND different category
            let mut choice = available
                .iter()
                .filter(|s| {
                    Some(s.file) != last_file
                        && last_category.map_or(true, |lc| s.category != lc)
                })
                .nth(pick_counter % available.len().max(1));

            // Fallback: just different file
            if choice.is_none() {
                choice = available
                    .iter()
                    .filter(|s| Some(s.file) != last_file)
                    .nth(pick_counter % available.len().max(1));
            }

            // Final fallback: rotate through all available
            choice.unwrap_or(&available[pick_counter % available.len()])
        };
        pick_counter += 1;

        // Compute priority (determines at what slider level this gets included)
        let priority = (density_mult * tempo_mult * candidate.beat_strength as f64)
            .clamp(0.0, 1.0) as f32;

        // Compute pitch shift
        let pitch_shift = if sample.pitched {
            if let Some(sample_pc) = sample.estimated_pitch_class {
                let key_seg = key_segments
                    .iter()
                    .find(|k| candidate.time >= k.start_seconds && candidate.time < k.end_seconds)
                    .or(key_segments.first());

                if let Some(ks) = key_seg {
                    // Target: root or 5th of current key
                    let root = ks.root_pitch_class;
                    let fifth = (root + 7) % 12;

                    let shift_to_root = ((root as i8 - sample_pc as i8) + 12) % 12;
                    let shift_to_fifth = ((fifth as i8 - sample_pc as i8) + 12) % 12;

                    // Pick smallest shift
                    let s1 = if shift_to_root > 6 { shift_to_root - 12 } else { shift_to_root };
                    let s2 = if shift_to_fifth > 6 { shift_to_fifth - 12 } else { shift_to_fifth };

                    if s1.abs() <= s2.abs() && s1.abs() <= 4 {
                        s1
                    } else if s2.abs() <= 4 {
                        s2
                    } else {
                        0 // Don't shift if it would be too extreme
                    }
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        // Gain based on section type
        let gain = match section_type {
            SectionType::Intro => 0.3,
            SectionType::Verse => 0.5,
            SectionType::Chorus => 0.7,
            SectionType::Bridge => 0.6,
            SectionType::Outro => 0.8,
        } as f32;

        // Add micro-timing swing for live feel
        let swing_ms = if tempo < 120.0 {
            // Simple deterministic swing based on position
            let phase = (candidate.time * 17.3).sin(); // pseudo-random from time
            0.010 + 0.020 * (phase * 0.5 + 0.5) // 10-30ms
        } else {
            0.0
        };

        placements.push(AdLibPlacement {
            time_seconds: candidate.time + swing_ms,
            sample_file: sample.file.to_string(),
            pitch_shift_semitones: pitch_shift,
            gain,
            priority,
        });

        last_category = Some(sample.category);
        last_file = Some(sample.file);
        last_time = candidate.time;
    }

    // Sort by priority descending so JS can easily slice by slider value
    placements.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap());
    placements
}
