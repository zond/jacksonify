# Ad-Lib Placement Algorithm: Design Document

## Goal

Given an uploaded song (mono f32 PCM), determine a set of time-stamped
**placement points** where MJ-style vocal ad-libs should be inserted, each
annotated with:

- The **sample** to use (from `samples/`).
- The **pitch shift** (in semitones) needed to match the local key/chord.
- A **gain** value (to control density escalation through the song).

The algorithm runs entirely client-side in Rust/WASM, building on the existing
`beat.rs` and `key.rs` modules.

---

## 1. Overview of the Pipeline

```
PCM samples (mono, f32)
        |
        v
+---------------------+
| Phase 1: Foundation  |  (existing code, extended)
|  - Beat grid         |
|  - Global key        |
+---------------------+
        |
        v
+---------------------+
| Phase 2: Structure   |  (new)
|  - Energy envelope   |
|  - Section detector  |
|  - Vocal gap finder  |
+---------------------+
        |
        v
+---------------------+
| Phase 3: Placement   |  (new)
|  - Candidate grid    |
|  - Rule application  |
|  - Density control   |
|  - Pitch selection   |
+---------------------+
        |
        v
Vec<AdLibPlacement>
```

---

## 2. Phase 1 -- Foundation (Existing Code, Extended)

### 2A. Beat Grid Construction

**What exists:** `beat::detect_tempo()` returns a single BPM number. Internally
it computes spectral flux, picks onset peaks, and histogram-votes for a tempo.

**What is needed:** A full **beat grid** -- an ordered list of timestamps for
every beat in the song, with sub-beat subdivisions.

**Approach:**

1. **Reuse `compute_spectral_flux()` and `pick_peaks()` as-is.** These are well
   structured and produce a usable onset signal.

2. **Add `detect_beat_grid(samples, sample_rate) -> BeatGrid`**, a new public
   function in `beat.rs` that:
   a. Calls the existing functions to get tempo.
   b. Uses autocorrelation on the onset signal to find the **phase** (offset of
      beat 1). The autocorrelation lag corresponding to the detected BPM gives
      the beat period in frames; the peak position within that period gives the
      phase.
   c. Generates a uniform grid: `beat_times[i] = phase + i * beat_period`.
   d. Refines each grid point by snapping to the nearest onset peak within a
      tolerance window (e.g. +/- 15% of beat period), to handle tempo drift.

3. **Sub-beat positions.** From each beat timestamp, derive:
   - The "&" (upbeat) at `beat + 0.5 * beat_period`
   - 16th-note positions at `beat + 0.25 * beat_period` ("e") and
     `beat + 0.75 * beat_period` ("a")

   These subdivisions are not snapped to onsets; they are purely interpolated
   from the grid.

4. **Downbeat estimation.** To know which beats are 1, 2, 3, 4 (assuming 4/4
   time):
   - Compute the RMS energy at each beat grid point over a ~50ms window.
   - Beats with strongest bass energy (low-pass below 150 Hz before measuring)
     tend to be beats 1 and 3 (kick drum).
   - Beats with strongest mid-band energy (200-1000 Hz) tend to be beats 2 and
     4 (snare).
   - Group beats into fours. Score each possible rotation (offset 0-3) by how
     well the bass/mid pattern matches the expected
     kick-snare-kick-snare pattern. Pick the rotation with the highest score.
   - This gives us labeled beats: 1, 2, 3, 4, 1, 2, 3, 4, ...

**Data structure:**

```rust
pub struct BeatGrid {
    pub tempo_bpm: f64,
    pub beats: Vec<BeatInfo>,
}

pub struct BeatInfo {
    pub time_seconds: f64,    // timestamp of this beat
    pub beat_in_bar: u8,      // 1, 2, 3, or 4
    pub bar_number: u32,      // which bar (0-indexed)
}

impl BeatGrid {
    pub fn upbeat_after(&self, idx: usize) -> f64 { ... }
    pub fn sixteenth_e(&self, idx: usize) -> f64 { ... }
    pub fn sixteenth_a(&self, idx: usize) -> f64 { ... }
}
```

### 2B. Key Detection (Windowed)

**What exists:** `key::detect_key()` computes a single global chromagram and
returns one key for the entire song.

**What is needed:** A **time-varying key** to handle modulations (the reference
doc notes ~25-30% of MJ songs modulate, usually upward by a half step in the
final 20-30%).

**Approach:**

1. **Reuse `compute_chromagram()` logic** but wrap it in a windowed version.

2. **Add `detect_key_segments(samples, sample_rate, window_sec) -> Vec<KeySegment>`**:
   a. Slide a window (e.g. 8 seconds, hop 4 seconds) across the audio.
   b. For each window, compute a local chromagram using the existing
      FFT/binning code.
   c. Correlate with Krumhansl-Schmuckler profiles (existing
      `correlate_with_profiles()`).
   d. Record the best key and confidence for each window.
   e. Merge consecutive windows with the same key into segments.
   f. Discard very short segments (< 4 seconds) as noise, assigning them the
      surrounding key.

3. **Modulation detection.** After merging, check for the MJ pattern: if the
   last segment is a half step or whole step above the previous segment, flag it
   as a modulation point. This timestamp is useful because ad-lib density
   typically spikes at modulations.

**Data structure:**

```rust
pub struct KeySegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub key: String,          // e.g. "Eb major"
    pub root_pitch_class: u8, // 0-11 (C=0)
    pub is_minor: bool,
    pub confidence: f64,
}
```

---

## 3. Phase 2 -- Structural Analysis (New Module: `structure.rs`)

This phase answers: "Where are the verses, choruses, and outros?" and "Where
are the vocal gaps?"

### 3A. Energy Envelope

Compute a smoothed RMS energy curve over the entire track:

1. Calculate RMS in 100ms windows (hop 50ms).
2. Apply a median filter (window ~1s) to remove transient spikes.
3. Normalize to [0, 1].

This gives a coarse loudness contour. Choruses are louder than verses;
intros/outros may be quieter.

### 3B. Spectral Contrast Envelope

To distinguish sections more reliably than energy alone:

1. For each 100ms window, compute the ratio of high-frequency energy
   (above 4kHz) to total energy. Choruses typically have brighter, denser
   instrumentation.
2. Also compute the spectral centroid (already have FFT infrastructure).
3. Combine into a "brightness" curve.

### 3C. Self-Similarity Matrix (SSM) for Section Detection

This is the core of song structure analysis and is well studied in MIR
(Music Information Retrieval).

**Algorithm:**

1. **Feature extraction:** For each beat (using the beat grid from Phase 1),
   compute a 12-dimensional chroma vector plus a few timbral features (spectral
   centroid, RMS, spectral rolloff). This yields a ~15-dimensional feature
   vector per beat.

2. **Self-similarity matrix:** Compute cosine similarity between every pair of
   beat-aligned feature vectors. This produces an N x N matrix where N = number
   of beats. Repeated sections (e.g. chorus1 vs chorus2) appear as bright
   off-diagonal stripes.

3. **Novelty curve:** Convolve a checkerboard kernel along the main diagonal of
   the SSM. Peaks in this novelty curve correspond to section boundaries.
   The checkerboard kernel has the form:
   ```
   [+1 -1]
   [-1 +1]
   ```
   (scaled to a size like 16x16 beats, i.e. ~4 bars at the detected tempo).

4. **Peak picking on the novelty curve** gives boundary timestamps. Combine
   with the following heuristics:
   - Minimum section length: 8 bars (~16-32 seconds depending on tempo).
   - Maximum section length: 32 bars.
   - Boundaries should be close to bar lines (snap to nearest bar start).

5. **Section labeling:** For each section, compare its average feature vector to
   every other section's average. Sections with high mutual similarity get the
   same label (A, B, C...). Then apply heuristics:
   - The first occurrence of label A is likely "verse" (usually the first full
     section after the intro).
   - The first occurrence of label B is likely "chorus" (typically louder/
     brighter than A).
   - The last section, if it repeats a chorus label but with higher energy or
     if it fades out, is "outro".
   - A one-off section between repeated sections is "bridge".
   - A low-energy opening section is "intro".

**Data structure:**

```rust
pub enum SectionType {
    Intro,
    Verse,
    PreChorus,
    Chorus,
    Bridge,
    Outro,
    Unknown,
}

pub struct Section {
    pub section_type: SectionType,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub energy: f64,         // average RMS
    pub brightness: f64,     // average spectral centroid ratio
    pub label: char,         // structural label (A, B, C...)
    pub occurrence: u32,     // 1st, 2nd, 3rd time this label appears
}
```

### 3D. Vocal Gap Detection

Ad-libs go in the spaces between vocal phrases. We need to find these gaps
without a vocal separation model (which would be too heavy for WASM).

**Approach: Spectral band energy heuristic.**

Human voice dominant energy sits in ~300Hz-3kHz. The presence of sustained
energy in this band with a clear harmonic series indicates vocals. Gaps are
periods where this band energy drops.

1. **Band-pass filter** the audio to 300-3000 Hz (apply in frequency domain
   using the existing FFT pipeline -- zero out bins outside the range before
   IFFT, or simply measure energy only in those bins without reconstructing).

2. **Compute per-frame energy** in the vocal band (frame size ~23ms / 1024
   samples at 44.1kHz, hop ~12ms).

3. **Compute a "vocal activity" signal:** ratio of vocal-band energy to
   full-band energy. When instruments are playing but no voice is present, this
   ratio drops because the energy is spread more evenly.

4. **Threshold and smooth:**
   - Apply a running median (window ~200ms) to remove brief dips.
   - Set a threshold: frames below the threshold are "gaps".
   - Require minimum gap duration of ~200ms (shorter gaps are too brief for an
     ad-lib).
   - Require maximum gap duration of ~2 seconds (longer gaps are probably
     instrumental breaks where ad-libs would sound wrong, unless it is an
     outro).

5. **Output:** A list of gap intervals `(start_seconds, end_seconds)`.

**Improvement -- onset density as a secondary signal:**

- In gaps, there are fewer onsets (no consonant attacks from vocals).
- Combine low vocal-band energy with low onset density in the 1-4kHz band to
  increase confidence.

**Data structure:**

```rust
pub struct VocalGap {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub duration: f64,
    pub confidence: f64,  // how certain we are this is a true gap
}
```

---

## 4. Phase 3 -- Ad-Lib Placement (New Module: `placement.rs`)

This is where all the pieces come together.

### 4A. Sample Taxonomy

First, classify each sample in `samples/` by type so the algorithm knows which
rhythmic and harmonic rules to apply.

```rust
pub enum AdLibCategory {
    HeeHee,       // hee-hee.mp3, eeeehe.mp3
    Shout,        // yow.mp3, ieow.mp3, gadaaao.mp3
    Grunt,        // ah.mp3, aeh.mp3, eh.mp3, uh-uh.mp3, uh-duh.mp3
    Gasp,         // haaaaoo.mp3, ooooh.mp3, ooouuh.mp3
    Moan,         // aaah.mp3, eeeh.mp3, eeh.mp3, ouh-hou.mp3
    Shamone,      // shamone.mp3, shaka.mp3, shaka-shaka.mp3
    MelodicPhrase,// be-careful-with-what-you-do.mp3, yaaa-dont-baaaby.mp3,
                  // swear-dont-maybe.mp3, dah-dah-dah.mp3
    Exclamation,  // yeeahaaa.mp3, yeeeah.mp3, uooou.mp3,
                  // ooooou-hooooou.mp3, fucking-dem-bea.mp3
}
```

Each category has pre-defined placement rules (see 4B) and a flag indicating
whether it is pitched (needs pitch shifting) or unpitched.

For each sample file, pre-analyze and store:
- Duration in seconds.
- Whether it is pitched, and if so, its detected root pitch (use the existing
  chromagram on the sample itself -- a short sample with a dominant pitch will
  produce a chromagram with one very strong bin).
- Its category (hardcoded mapping from filename).

### 4B. Candidate Grid Generation

For every beat in the beat grid, generate **candidate placement slots**
according to the rules from the reference doc:

| Slot position         | Candidate ad-lib categories    | Beat requirement      |
|-----------------------|--------------------------------|-----------------------|
| & of beats 2, 4       | HeeHee                         | Upbeat, weak beat     |
| Beat 1, beat 3        | Shout, Exclamation             | Downbeat, strong beat |
| & of beat 4            | Gasp                           | Pre-downbeat          |
| Beat 2, beat 4         | Grunt                          | Backbeat              |
| "e" or "a" (16ths)    | HeeHee (at high tempos)        | Fast subdivision      |
| Any gap center         | Moan, Shamone, MelodicPhrase   | Between vocal phrases |

Algorithm:

```
for each beat in beat_grid:
    match beat.beat_in_bar:
        1 | 3 => add Candidate(beat.time, [Shout, Exclamation])
        2 | 4 => add Candidate(beat.time, [Grunt])

    // Upbeats
    let upbeat = beat_grid.upbeat_after(beat_idx)
    match beat.beat_in_bar:
        2 | 4 => add Candidate(upbeat, [HeeHee])

    // Pre-downbeat gasp
    if beat.beat_in_bar == 4:
        add Candidate(upbeat, [Gasp])   // & of 4

    // 16th notes (only if tempo > 110 BPM)
    if tempo > 110:
        add Candidate(beat_grid.sixteenth_e(beat_idx), [HeeHee])
        add Candidate(beat_grid.sixteenth_a(beat_idx), [HeeHee])
```

This produces a **large** list of candidates -- far too many. The following
filters thin it out.

### 4C. Gap Filter

Remove any candidate whose timestamp does NOT fall inside a detected vocal gap.
MJ's ad-libs sit in the spaces between lyrical phrases, not on top of them.

Exception: Grunts and gasps can overlap slightly with vocal phrases (they are
percussive and short), so allow candidates of those types if they are within
50ms of a gap boundary.

### 4D. Density Control via Song Structure

The reference doc specifies escalation:

```
Intro:  0-1 ad-libs per 8 bars
Verse:  1-2 ad-libs per 8 bars
Chorus: 3-6 ad-libs per 8 bars
Bridge: 2-4 ad-libs per 8 bars
Outro:  6-12 ad-libs per 8 bars
```

Also: each successive chorus should have more ad-libs than the previous one.

**Algorithm:**

1. For each section (from the structure detector), assign a **density target**
   (ad-libs per 8 bars) based on section type and occurrence number.
   - `base_density = match section_type { Intro => 0.5, Verse => 1.5, ... }`
   - `escalation_factor = 1.0 + 0.2 * (occurrence - 1)` (each repeat of a
     section type is 20% denser).
   - `target = base_density * escalation_factor`

2. Count the candidates remaining (after gap filter) within each section.

3. If there are more candidates than the target allows, randomly (seeded for
   reproducibility) select a subset, preferring candidates that are:
   - At rhythmically stronger positions (downbeats > upbeats > 16ths).
   - In longer gaps (more room for the ad-lib to breathe).
   - More diverse in type (avoid three hee-hees in a row).

4. Tempo-based rate limiting (from the reference doc):
   - Below 90 BPM: cap at 2 ad-libs per minute. Prefer breathy/melodic types.
   - 90-110 BPM: cap at 15 per minute. Full variety.
   - Above 110 BPM: allow up to 40 per minute. Prefer percussive types.

### 4E. Pitch Selection for Pitched Ad-Libs

For each placed ad-lib that is pitched (HeeHee, Shout, Moan, MelodicPhrase,
Exclamation):

1. Look up the active `KeySegment` at the ad-lib's timestamp.
2. Determine the target pitch class based on the ad-lib type (from the reference
   doc):
   - HeeHee: root or 5th of current key.
   - Shout/Ow: root or b7.
   - Moan/melodic: root, 3rd, or 5th.

3. The sample has a known original pitch class (pre-analyzed). Compute the
   semitone shift:
   ```
   shift = (target_pitch_class - sample_pitch_class + 12) % 12
   if shift > 6 { shift -= 12 }  // prefer smallest shift
   ```

4. Constrain the shift to +/- 4 semitones to avoid unnatural pitch artifacts.
   If the required shift exceeds this, try alternative target pitch classes
   (e.g. fall back from root to 5th).

5. For the MJ-specific register: HeeHee samples should land in Bb4-Eb5 range,
   shouts in F3-F4 range. Apply octave shifts if needed (shift +/- 12
   semitones) to land in the right register.

### 4F. Micro-timing (Swing/Humanization)

The reference doc notes 10-40ms behind-the-beat swing on live-feel tracks:

1. If the detected tempo is below 120 BPM (more likely a live-feel groove),
   delay each ad-lib placement by a random 10-30ms.
2. If the tempo is above 120 BPM (more likely programmed), snap to exact grid
   (no offset).
3. Use a seeded PRNG for reproducibility.

### 4G. Output Structure

```rust
pub struct AdLibPlacement {
    pub time_seconds: f64,       // when to start playback
    pub sample_file: String,     // e.g. "hee-hee.mp3"
    pub pitch_shift_semitones: i8,
    pub gain: f32,               // 0.0 - 1.0
    pub category: AdLibCategory,
    pub beat_position: String,   // e.g. "& of 4", "beat 1"
    pub section: String,         // e.g. "Chorus 2"
}
```

---

## 5. What Existing Code Can Be Reused

| Existing function                  | Reuse                                         |
|------------------------------------|-----------------------------------------------|
| `beat::compute_spectral_flux()`    | Directly, as-is                               |
| `beat::pick_peaks()`              | Directly, as-is                               |
| `beat::estimate_tempo()`          | Directly (for BPM number); augmented with phase detection |
| `key::compute_chromagram()`       | Refactored into a windowed version; inner loop reused verbatim |
| `key::correlate_with_profiles()`  | Directly, as-is                               |
| `key::pearson_correlation()`      | Directly, as-is                               |
| `key::rotate_profile()`          | Directly, as-is                               |
| FFT setup / Hann window           | Shared across all new analysis functions       |

The existing `FRAME_SIZE` / `HOP_SIZE` constants in `beat.rs` (1024/512) are
appropriate for onset detection. The ones in `key.rs` (4096/2048) are
appropriate for pitch/chroma work. Both will be used in their respective
contexts.

---

## 6. New Modules and Functions Required

### `src/beat.rs` -- Extensions

```
pub fn detect_beat_grid(samples, sample_rate) -> BeatGrid
fn detect_phase(onset_signal, tempo_bpm, sample_rate) -> f64
fn estimate_downbeat(samples, beat_grid, sample_rate) -> Vec<u8>
```

### `src/key.rs` -- Extensions

```
pub fn detect_key_segments(samples, sample_rate, window_sec) -> Vec<KeySegment>
pub fn detect_pitch_class(samples, sample_rate) -> (u8, f64)
    // For analyzing individual short samples
```

### `src/structure.rs` -- New Module

```
pub fn analyze_structure(samples, sample_rate, beat_grid) -> Vec<Section>
fn compute_energy_envelope(samples, sample_rate) -> Vec<f64>
fn compute_beat_features(samples, sample_rate, beat_grid) -> Vec<FeatureVector>
fn self_similarity_matrix(features) -> Vec<Vec<f64>>
fn novelty_curve(ssm) -> Vec<f64>
fn label_sections(boundaries, features) -> Vec<Section>
```

### `src/gaps.rs` -- New Module

```
pub fn detect_vocal_gaps(samples, sample_rate) -> Vec<VocalGap>
fn vocal_band_energy(samples, sample_rate) -> Vec<f64>
```

### `src/placement.rs` -- New Module

```
pub fn place_ad_libs(
    beat_grid: &BeatGrid,
    key_segments: &[KeySegment],
    sections: &[Section],
    gaps: &[VocalGap],
    sample_catalog: &SampleCatalog,
) -> Vec<AdLibPlacement>

fn generate_candidates(beat_grid, tempo) -> Vec<Candidate>
fn filter_by_gaps(candidates, gaps) -> Vec<Candidate>
fn apply_density_control(candidates, sections, tempo) -> Vec<Candidate>
fn assign_samples(candidates, catalog) -> Vec<AdLibPlacement>
fn compute_pitch_shift(sample_pitch, target_pitch, category) -> i8
fn apply_micro_timing(placements, tempo) -> Vec<AdLibPlacement>
```

### `src/samples.rs` -- New Module

```
pub struct SampleCatalog { ... }
pub fn build_catalog() -> SampleCatalog
    // Hardcoded mapping of filenames -> categories, durations, pitches
```

### `src/lib.rs` -- Extended API

```
pub fn jacksonify(samples: &[f32], sample_rate: f32) -> JacksonifyResult
    // Returns AnalysisResult + Vec<AdLibPlacement>
```

---

## 7. Computational Complexity and WASM Considerations

### Most expensive operations:

1. **Self-similarity matrix:** O(N^2) where N = number of beats. A 4-minute
   song at 120 BPM has ~480 beats, so the matrix is 480x480 = 230K entries.
   Each entry is a dot product of ~15-dimensional vectors. This is very
   manageable.

2. **FFT for chromagram / spectral flux:** Already done in existing code.
   Windowed key detection adds more FFT passes but the window hop is large
   (4 seconds), so it is a small number of extra passes.

3. **Vocal gap detection:** One extra pass over the audio with FFT, same
   complexity as existing spectral flux.

**Estimated total cost:** Roughly 3x the current analysis time, which should
still be under 2 seconds for a typical 3-4 minute song on modern hardware
running WASM.

### Memory:

- The self-similarity matrix for 480 beats x 480 beats x 8 bytes = ~1.8 MB.
- All other intermediate data is small.
- Total memory overhead is well within WASM's typical 256 MB-4 GB limits.

### No new dependencies required.

All algorithms use FFT (already have `rustfft`) and basic linear algebra
(dot products, Pearson correlation -- already implemented). No ML models,
no audio separation networks, no new crates needed. The `js-sys` crate already
in `Cargo.toml` can provide `Math.random()` for seeded selection if needed, but
a simple LCG PRNG in Rust would be lighter.

---

## 8. Algorithm Accuracy Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Beat grid phase error (beat 1 detected as beat 3) | Wrong ad-lib types on wrong beats | Use bass/mid energy ratio for downbeat estimation; allow user override |
| Section detection fails on unusual structures | Density control breaks down | Fall back to energy-only heuristic (louder = more ad-libs) |
| Vocal gap detection picks up instruments as vocals | Ad-libs placed on top of singing | Require gaps to be at least 200ms and confirmed by low onset density |
| Key detection wrong in a segment | Pitched ad-libs sound dissonant | Use confidence threshold; fall back to root of global key if local confidence is low |
| Tempo too slow or fast for ad-lib style | Unnatural placement | Enforce the tempo-based rate limits from the reference doc; at extreme tempos (< 70 or > 160), reduce density heavily |

---

## 9. Future Enhancements (Out of Scope for Initial Implementation)

1. **User controls:** Allow the user to adjust density (slider from "subtle" to
   "maximum MJ"), prefer certain ad-lib types, or manually mark section
   boundaries.

2. **Chord-aware pitch selection:** Instead of just the key root/5th, use a
   local chord detection (chroma-based) to pick ad-lib pitches that match the
   current chord. This would require a chord template matching step on the
   windowed chromagram.

3. **Sample onset alignment:** Some samples have silence or a breath at the
   start. Pre-trim samples to their onset for tighter rhythmic placement.

4. **Vocal separation via spectral masking:** A lightweight
   harmonic/percussive source separation (HPSS) could improve gap detection.
   HPSS only requires median filtering of the spectrogram (no ML) and could
   run in WASM.

5. **Style era selection:** The reference doc distinguishes Off the Wall-era
   (lighter, falsetto-dominant) from Dangerous-era (aggressive, grunt-heavy).
   Let the user pick an era, which adjusts the sample selection weights and
   density profiles.

---

## 10. Summary of the End-to-End Flow

1. User uploads audio. JavaScript decodes it to mono f32 PCM via Web Audio API.
2. Call `jacksonify(samples, sample_rate)` in WASM.
3. **Beat grid:** spectral flux -> onset peaks -> tempo -> phase -> grid ->
   downbeat labeling.
4. **Key segments:** windowed chromagram -> Krumhansl-Schmuckler per window ->
   merged segments.
5. **Structure:** beat-aligned features -> SSM -> novelty curve -> boundaries ->
   section labeling.
6. **Vocal gaps:** vocal-band energy -> threshold -> gap intervals.
7. **Candidate generation:** beat grid positions x ad-lib type rules.
8. **Filtering:** intersect with gaps, apply section density, tempo rate limits.
9. **Assignment:** pick specific samples, compute pitch shifts, apply
   micro-timing.
10. Return `Vec<AdLibPlacement>` to JavaScript for playback scheduling.
