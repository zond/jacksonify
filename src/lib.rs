use wasm_bindgen::prelude::*;

mod beat;
mod key;

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
///
/// `samples` should be mono f32 PCM data, `sample_rate` in Hz (e.g. 44100).
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
