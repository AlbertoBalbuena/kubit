//! wasm32 probe for the monstertruck backend (spike test 14).

use backend_monstertruck::Monstertruck;
use harness::{corpus, measure, Backend, SpikeError};
use serde_json::json;
use wasm_bindgen::prelude::wasm_bindgen;

/// Runs one corpus case and returns a JSON summary.
#[wasm_bindgen]
pub fn run_case(id: &str) -> String {
    let Some(case) = corpus::find(id) else {
        return json!({ "id": id, "status": "unknown_case" }).to_string();
    };
    match Monstertruck.run(id, 0.01) {
        Ok(p) => {
            let eval = measure::evaluate(
                p.mesh.as_ref(),
                p.topo.as_ref(),
                p.native_volume,
                p.native_area,
                &case.oracle,
            );
            json!({
                "id": id,
                "status": "ok",
                "volume": eval.volume,
                "volume_rel_err": eval.volume_rel_err,
                "valid": eval.valid,
                "step_bytes": p.step.as_ref().map(String::len),
            })
            .to_string()
        }
        Err(SpikeError::NotExposed(e)) => {
            json!({ "id": id, "status": "not_exposed", "error": e }).to_string()
        }
        Err(SpikeError::Failed(e)) => {
            json!({ "id": id, "status": "failed", "error": e }).to_string()
        }
    }
}
