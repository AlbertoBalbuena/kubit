//! Spike adapter for the arris kernel. See `spikes/kernel/CASES.md` for the
//! geometry of every case and `backends/arris/NOTES.md` for the API used.

use harness::{not_exposed, Backend, Hostile, Produced, SpikeResult};

pub struct Arris;

impl Backend for Arris {
    fn name(&self) -> &'static str {
        "arris"
    }

    fn run(&self, case_id: &str, _tol: f64) -> SpikeResult<Produced> {
        not_exposed(&format!("adapter not written yet ({case_id})"))
    }

    fn import_step(&self, _text: &str, _tol: f64) -> SpikeResult<Produced> {
        not_exposed("adapter not written yet (STEP import)")
    }

    fn hostile(&self, _family: Hostile, _params: &[f64]) -> SpikeResult<()> {
        not_exposed("adapter not written yet (hostile)")
    }
}
