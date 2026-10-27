//! Spike adapter for the solidcore kernel. See `spikes/kernel/CASES.md` for the
//! geometry of every case and `backends/solidcore/NOTES.md` for the API used.

use harness::{not_exposed, Backend, Hostile, Produced, SpikeResult};

pub struct Solidcore;

impl Backend for Solidcore {
    fn name(&self) -> &'static str {
        "solidcore"
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
