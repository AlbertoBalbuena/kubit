//! wasm32 probe for the arris backend (test 14). See `spikes/kernel/wasm/README.md`.

use backend_arris::Arris;
use harness::{corpus, measure, Backend, SpikeError};
use serde_json::json;
use wasm_bindgen::prelude::wasm_bindgen;

/// Runs one corpus case at chord tolerance 0.01 and returns a JSON summary.
#[wasm_bindgen]
pub fn run_case(id: &str) -> String {
    let summary = match Arris.run(id, 0.01) {
        Ok(p) => {
            let oracle = corpus::find(id).map(|c| c.oracle).unwrap_or_default();
            let e = measure::evaluate(p.mesh.as_ref(), p.topo.as_ref(), p.native_volume, p.native_area, &oracle);
            json!({
                "id": id,
                "status": "ok",
                "volume": e.volume,
                "volume_rel_err": e.volume_rel_err,
                "valid": e.valid,
            })
        }
        Err(SpikeError::NotExposed(msg)) => json!({ "id": id, "status": "not_exposed", "error": msg }),
        Err(SpikeError::Failed(msg)) => json!({ "id": id, "status": "failed", "error": msg }),
    };
    summary.to_string()
}
