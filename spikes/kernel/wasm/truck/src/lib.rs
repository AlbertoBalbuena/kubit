//! wasm32 probe for the truck adapter (test 14). `run_case` runs one corpus case and
//! returns a JSON summary; a Rust panic surfaces in JS as `RuntimeError: unreachable`.

use harness::corpus;
use harness::measure::evaluate;
use harness::{Backend, SpikeError};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run_case(id: &str) -> String {
    let Some(case) = corpus::find(id) else {
        return serde_json::json!({ "status": "error", "message": format!("unknown case {id}") }).to_string();
    };
    let value = match backend_truck::Truck.run(id, 0.01) {
        Ok(p) => {
            let e = evaluate(p.mesh.as_ref(), p.topo.as_ref(), p.native_volume, p.native_area, &case.oracle);
            serde_json::json!({
                "status": "ok",
                "volume": e.volume,
                "volume_rel_err": e.volume_rel_err,
                "valid": e.valid,
            })
        }
        Err(SpikeError::NotExposed(m)) => serde_json::json!({ "status": "not_exposed", "message": m }),
        Err(SpikeError::Failed(m)) => serde_json::json!({ "status": "error", "message": m }),
    };
    value.to_string()
}
