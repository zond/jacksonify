use wasm_bindgen::prelude::*;

mod beat;
mod gaps;
mod key;
mod placement;
mod structure;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct AnalysisResult {
    tempo: f64,
    key: String,
    confidence: f64,
}

#[wasm_bindgen]
impl AnalysisResult {
    #[wasm_bindgen(getter)]
    pub fn tempo(&self) -> f64 {
        self.tempo
    }

    #[wasm_bindgen(getter)]
    pub fn key(&self) -> String {
        self.key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f64 {
        self.confidence
    }
}

/// Analyze audio samples and return detected tempo (BPM) and musical key.
#[wasm_bindgen]
pub fn analyze(samples: &[f32], sample_rate: f32) -> AnalysisResult {
    let tempo = beat::detect_tempo(samples, sample_rate);
    let (key_name, confidence) = key::detect_key(samples, sample_rate);

    AnalysisResult {
        tempo,
        key: key_name,
        confidence,
    }
}

// --- Jacksonify API ---

#[wasm_bindgen]
pub struct PlacementEntry {
    time_seconds: f64,
    sample_file: String,
    pitch_shift: i8,
    playback_rate: f32,
    gain: f32,
    priority: f32,
}

#[wasm_bindgen]
impl PlacementEntry {
    #[wasm_bindgen(getter)]
    pub fn time_seconds(&self) -> f64 {
        self.time_seconds
    }
    #[wasm_bindgen(getter)]
    pub fn sample_file(&self) -> String {
        self.sample_file.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn pitch_shift(&self) -> i8 {
        self.pitch_shift
    }
    #[wasm_bindgen(getter)]
    pub fn playback_rate(&self) -> f32 {
        self.playback_rate
    }
    #[wasm_bindgen(getter)]
    pub fn gain(&self) -> f32 {
        self.gain
    }
    #[wasm_bindgen(getter)]
    pub fn priority(&self) -> f32 {
        self.priority
    }
}

#[wasm_bindgen]
pub struct JacksonifyResult {
    tempo: f64,
    key: String,
    confidence: f64,
    placements: Vec<PlacementEntry>,
}

#[wasm_bindgen]
impl JacksonifyResult {
    #[wasm_bindgen(getter)]
    pub fn tempo(&self) -> f64 {
        self.tempo
    }
    #[wasm_bindgen(getter)]
    pub fn key(&self) -> String {
        self.key.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f64 {
        self.confidence
    }
    #[wasm_bindgen(getter)]
    pub fn placement_count(&self) -> usize {
        self.placements.len()
    }

    pub fn get_placement(&self, index: usize) -> Option<PlacementEntry> {
        self.placements.get(index).map(|p| PlacementEntry {
            time_seconds: p.time_seconds,
            sample_file: p.sample_file.clone(),
            pitch_shift: p.pitch_shift,
            playback_rate: p.playback_rate,
            gain: p.gain,
            priority: p.priority,
        })
    }
}

/// Full analysis: detect key, beat, song structure, and compute MJ ad-lib placements.
#[wasm_bindgen]
pub fn jacksonify(samples: &[f32], sample_rate: f32) -> JacksonifyResult {
    let beat_grid = beat::detect_beat_grid(samples, sample_rate);
    let (key_name, confidence) = key::detect_key(samples, sample_rate);
    let key_segments = key::detect_key_segments(samples, sample_rate, 8.0);
    let sections = structure::analyze_structure(samples, sample_rate, &beat_grid);
    let vocal_gaps = gaps::detect_vocal_gaps(samples, sample_rate);

    let raw_placements =
        placement::place_ad_libs(&beat_grid, &key_segments, &sections, &vocal_gaps);

    let placements: Vec<PlacementEntry> = raw_placements
        .into_iter()
        .map(|p| PlacementEntry {
            time_seconds: p.time_seconds,
            sample_file: p.sample_file,
            pitch_shift: p.pitch_shift_semitones,
            playback_rate: p.playback_rate,
            gain: p.gain,
            priority: p.priority,
        })
        .collect();

    JacksonifyResult {
        tempo: beat_grid.tempo_bpm,
        key: key_name,
        confidence,
        placements,
    }
}
