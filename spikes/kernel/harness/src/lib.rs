//! Backend-independent part of the kernel spike: corpus, result types and the
//! measurements every backend is judged by.

pub mod corpus;
pub mod hostile;
pub mod measure;

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Triangle mesh handed over by a backend. Positions in mm.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TriMesh {
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// B-rep topology counts as reported by the backend itself.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Topo {
    pub vertices: usize,
    pub edges: usize,
    pub faces: usize,
    /// Number of face boundary loops (outer + inner), needed for Euler–Poincaré.
    pub loops: Option<usize>,
    pub shells: usize,
    /// Backend's own closedness verdict, if it has one.
    pub backend_closed: Option<bool>,
    /// Backend's own validity checker verdict, if it has one.
    pub backend_valid: Option<bool>,
    /// Name of the check used (for the report).
    pub backend_check: Option<String>,
}

/// What the backend says about which faces an operation produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum History {
    /// The API returns explicit generated/modified/deleted information.
    Native { generated: usize, modified: usize, deleted: usize, detail: String },
    /// Something exists but does not cover generated/modified/deleted fully.
    Partial(String),
    /// The API returns nothing beyond the result shape.
    None,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Produced {
    pub mesh: Option<TriMesh>,
    pub topo: Option<Topo>,
    /// Volume from the backend's own mass properties, if exposed.
    pub native_volume: Option<f64>,
    pub native_area: Option<f64>,
    pub history: Option<History>,
    /// STEP text of the result, when the backend can write it.
    pub step: Option<String>,
    /// Error writing STEP, when it failed.
    pub step_error: Option<String>,
    /// Seconds spent in the modelling operations only (no tessellation/export).
    pub op_seconds: f64,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SpikeError {
    /// The backend has no public API for this operation.
    NotExposed(String),
    /// The operation returned an error (clean failure).
    Failed(String),
}

impl std::fmt::Display for SpikeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpikeError::NotExposed(s) => write!(f, "not exposed: {s}"),
            SpikeError::Failed(s) => write!(f, "failed: {s}"),
        }
    }
}

pub type SpikeResult<T> = Result<T, SpikeError>;

/// Hostile-input families for case 15.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Hostile {
    /// Case 1: box(a, b, c) − cylinder(r) through the middle. Params: [a, b, c, r].
    BoxMinusCylinder,
    /// Case 5: box(a, b, c), fillet one edge with radius r. Params: [a, b, c, r].
    FilletEdge,
    /// Case 9: box(a, b, c) with vertical fillets r, push top face by d. Params: [a, b, c, r, d].
    PushPull,
}

pub trait Backend {
    fn name(&self) -> &'static str;
    /// Runs one corpus case (`corpus::cases()` id) and returns its result.
    /// `tol` is the chord tolerance for the returned mesh.
    fn run(&self, case_id: &str, tol: f64) -> SpikeResult<Produced>;
    /// Imports a STEP file and returns the first solid (or all solids merged into one mesh).
    fn import_step(&self, text: &str, tol: f64) -> SpikeResult<Produced>;
    /// Runs a hostile-input family once with the given parameters.
    fn hostile(&self, family: Hostile, params: &[f64]) -> SpikeResult<()>;
}

/// Runs `f` and returns its value together with the elapsed seconds.
pub fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed().as_secs_f64())
}

pub fn not_exposed<T>(what: &str) -> SpikeResult<T> {
    Err(SpikeError::NotExposed(what.to_owned()))
}

pub fn failed<T>(what: impl Into<String>) -> SpikeResult<T> {
    Err(SpikeError::Failed(what.into()))
}
