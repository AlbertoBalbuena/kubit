//! wasm32 probe (test 14) for the solidcore backend: runs one corpus case and returns
//! a JSON summary. Timing is measured on the JS side (`run.mjs`).

use backend_solidcore::Solidcore;
use harness::{corpus, measure, Backend, SpikeError};
use serde_json::json;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn run_case(id: &str) -> String {
    let value = match Solidcore.run(id, 0.01) {
        Ok(p) => {
            let rel = corpus::find(id).and_then(|c| {
                measure::evaluate(p.mesh.as_ref(), p.topo.as_ref(), p.native_volume, p.native_area, &c.oracle)
                    .volume_rel_err
            });
            json!({"id": id, "status": "ok", "volume": p.native_volume, "volume_rel_err": rel})
        }
        Err(SpikeError::NotExposed(m)) => json!({"id": id, "status": "not_exposed", "message": m}),
        Err(SpikeError::Failed(m)) => json!({"id": id, "status": "failed", "message": m}),
    };
    value.to_string()
}
