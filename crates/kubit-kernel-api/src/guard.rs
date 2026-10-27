//! The guard: every kernel call goes through it (CLAUDE.md rule 3).
//!
//! It (1) rejects non-finite or out-of-range numbers before the backend sees them,
//! (2) turns a backend panic into `KernelError::Panicked` and poisons the kernel,
//! (3) validates every result with the backend's checker and a positive volume,
//! and (4) fills `OpHistory` by geometric matching when the backend has no
//! native history.
//!
//! Limitation: catching a panic needs `panic = "unwind"`. On
//! `wasm32-unknown-unknown` panics abort, so there the guard only does (1), (3)
//! and (4); the web build relies on backends that do not panic.

use crate::error::{KernelError, KernelResult};
use crate::history::{MatchTolerance, reconstruct_faces};
use crate::kernel::{Interrupt, Kernel, OpOutput};
use crate::types::{
    BodyId, BooleanOp, BoxSpec, CylinderSpec, EdgeId, FaceId, FaceInfo, KernelInfo, MAX_LENGTH,
    MIN_LENGTH, MassProperties, Profile, ProfileLoop, ProfileSeg, SurfaceKind, Validity,
};
use kubit_mesh::TriMesh;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Smallest and largest chord tolerance accepted for tessellation, mm.
pub const CHORD_RANGE: (f64, f64) = (1.0e-4, 10.0);

/// A kernel wrapped by the guard. Implements [`Kernel`] itself.
pub struct Guarded<K: Kernel> {
    inner: K,
    poisoned: bool,
    tolerance: MatchTolerance,
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

/// Checks that `v` is a finite length within ±`MAX_LENGTH` (and > `MIN_LENGTH` when `positive`).
pub fn check_length(param: &'static str, v: f64, positive: bool) -> KernelResult<f64> {
    let bad = |reason: String| Err(KernelError::InvalidInput { param, reason });
    if !v.is_finite() {
        return bad(format!("{v} is not finite"));
    }
    if v.abs() > MAX_LENGTH {
        return bad(format!("{v} mm exceeds ±{MAX_LENGTH} mm"));
    }
    if positive && v < MIN_LENGTH {
        return bad(format!("{v} mm must be at least {MIN_LENGTH} mm"));
    }
    Ok(v)
}

/// Checks every coordinate of a point.
pub fn check_point(param: &'static str, p: [f64; 3]) -> KernelResult<[f64; 3]> {
    for c in p {
        check_length(param, c, false)?;
    }
    Ok(p)
}

/// Normalises a direction, rejecting zero and non-finite vectors.
pub fn check_direction(param: &'static str, d: [f64; 3]) -> KernelResult<[f64; 3]> {
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !n.is_finite() || n < 1e-12 {
        return Err(KernelError::InvalidInput {
            param,
            reason: "zero or non-finite direction".into(),
        });
    }
    Ok([d[0] / n, d[1] / n, d[2] / n])
}

/// Most holes and most segments per loop accepted in a profile.
const MAX_PROFILE_HOLES: usize = 256;
const MAX_PROFILE_SEGMENTS: usize = 4096;
/// Slack for "unit length", "perpendicular" and "loop closes", in axis units / mm.
const PROFILE_SLACK: f64 = 1.0e-6;

fn bad_profile(param: &'static str, reason: impl Into<String>) -> KernelError {
    KernelError::InvalidInput {
        param,
        reason: reason.into(),
    }
}

fn check_uv(param: &'static str, p: [f64; 2]) -> KernelResult<[f64; 2]> {
    check_length(param, p[0], false)?;
    check_length(param, p[1], false)?;
    Ok(p)
}

fn uv_distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn check_unit_axis(param: &'static str, d: [f64; 3]) -> KernelResult<()> {
    check_point(param, d)?;
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if (n - 1.0).abs() > PROFILE_SLACK {
        return Err(bad_profile(param, format!("length {n} is not 1")));
    }
    Ok(())
}

fn check_profile_loop(param: &'static str, l: &ProfileLoop) -> KernelResult<()> {
    match l {
        ProfileLoop::Circle { center, radius } => {
            check_uv(param, *center)?;
            check_length(param, *radius, true)?;
        }
        ProfileLoop::Segments { start, segments } => {
            check_uv(param, *start)?;
            if segments.len() < 2 || segments.len() > MAX_PROFILE_SEGMENTS {
                return Err(bad_profile(
                    param,
                    format!(
                        "{} segments; need 2 to {MAX_PROFILE_SEGMENTS}",
                        segments.len()
                    ),
                ));
            }
            let mut at = *start;
            for (i, seg) in segments.iter().enumerate() {
                let to = match *seg {
                    ProfileSeg::LineTo(to) => check_uv(param, to)?,
                    ProfileSeg::ArcTo { to, via } => {
                        check_uv(param, via)?;
                        let to = check_uv(param, to)?;
                        if uv_distance(via, at) < MIN_LENGTH || uv_distance(via, to) < MIN_LENGTH {
                            return Err(bad_profile(
                                param,
                                format!("segment {i}: arc `via` coincides with an end"),
                            ));
                        }
                        to
                    }
                };
                if uv_distance(at, to) < MIN_LENGTH {
                    return Err(bad_profile(param, format!("segment {i} has zero length")));
                }
                at = to;
            }
            if uv_distance(at, *start) > PROFILE_SLACK {
                return Err(bad_profile(param, "loop does not return to its start"));
            }
        }
    }
    Ok(())
}

/// Validates a profile: finite and bounded numbers, unit perpendicular axes,
/// loops that are non-degenerate and closed. Self-intersection is left to the
/// backend (its result check still applies).
fn check_profile(p: &Profile) -> KernelResult<()> {
    check_point("origin", p.origin)?;
    check_unit_axis("x_axis", p.x_axis)?;
    check_unit_axis("normal", p.normal)?;
    let dot: f64 = p.x_axis.iter().zip(p.normal).map(|(a, b)| a * b).sum();
    if dot.abs() > PROFILE_SLACK {
        return Err(bad_profile(
            "normal",
            "x_axis and normal are not perpendicular",
        ));
    }
    if p.holes.len() > MAX_PROFILE_HOLES {
        return Err(bad_profile(
            "holes",
            format!("more than {MAX_PROFILE_HOLES} holes"),
        ));
    }
    check_profile_loop("outer", &p.outer)?;
    for hole in &p.holes {
        check_profile_loop("holes", hole)?;
    }
    Ok(())
}

impl<K: Kernel> Guarded<K> {
    /// Wraps a backend.
    pub fn new(inner: K) -> Self {
        Self {
            inner,
            poisoned: false,
            tolerance: MatchTolerance::default(),
        }
    }

    /// Whether a panic has poisoned the kernel.
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Replaces a (poisoned) backend with a fresh one; the engine then replays the document.
    pub fn reset(&mut self, inner: K) {
        self.inner = inner;
        self.poisoned = false;
    }

    /// Read access to the backend (for backend-specific diagnostics only).
    pub fn inner(&self) -> &K {
        &self.inner
    }

    fn call_mut<T>(
        &mut self,
        op: &'static str,
        f: impl FnOnce(&mut K) -> KernelResult<T>,
    ) -> KernelResult<T> {
        if self.poisoned {
            return Err(KernelError::Poisoned);
        }
        match catch_unwind(AssertUnwindSafe(|| f(&mut self.inner))) {
            Ok(r) => r,
            Err(payload) => {
                self.poisoned = true;
                Err(KernelError::Panicked {
                    op,
                    message: panic_message(payload.as_ref()),
                })
            }
        }
    }

    fn call_ref<T>(
        &self,
        op: &'static str,
        f: impl FnOnce(&K) -> KernelResult<T>,
    ) -> KernelResult<T> {
        if self.poisoned {
            return Err(KernelError::Poisoned);
        }
        // A panic in a query cannot be recorded as poison through `&self`; the
        // query only reads, so the backend state is as consistent as before.
        catch_unwind(AssertUnwindSafe(|| f(&self.inner))).unwrap_or_else(|payload| {
            Err(KernelError::Panicked {
                op,
                message: panic_message(payload.as_ref()),
            })
        })
    }

    fn face_infos(&self, body: BodyId) -> KernelResult<Vec<FaceInfo>> {
        let faces = self.call_ref("faces", |k| k.faces(body))?;
        faces
            .into_iter()
            .map(|f| self.call_ref("face_info", |k| k.face_info(f)))
            .collect()
    }

    /// Validates the result of `op` and fills its history; deletes it if invalid.
    fn finish(
        &mut self,
        op: &'static str,
        mut out: OpOutput,
        before: Vec<FaceInfo>,
        moved: &[(FaceId, f64)],
    ) -> KernelResult<OpOutput> {
        let verdict = self.validate(op, out.body);
        if let Err(e) = verdict {
            let _ = self.call_mut("delete_body", |k| k.delete_body(out.body));
            return Err(e);
        }
        if !self
            .call_ref("info", |k| Ok(k.info()))?
            .capabilities
            .native_history
        {
            let after = self.face_infos(out.body)?;
            let pairs = moved_pairs(&before, &after, moved, self.tolerance);
            out.history = reconstruct_faces(&before, &after, &pairs, self.tolerance);
        }
        Ok(out)
    }

    fn validate(&mut self, op: &'static str, body: BodyId) -> KernelResult<()> {
        let validity = self.call_ref(op, |k| k.check(body))?;
        if !validity.ok {
            return Err(KernelError::InvalidResult {
                op,
                reason: validity.issues.join("; "),
            });
        }
        let mass = self.call_ref(op, |k| k.mass_properties(body))?;
        if !(mass.volume.is_finite() && mass.volume > 0.0) {
            return Err(KernelError::InvalidResult {
                op,
                reason: format!("volume {} is not positive", mass.volume),
            });
        }
        Ok(())
    }

    fn before(&self, bodies: &[BodyId]) -> KernelResult<Vec<FaceInfo>> {
        let mut out = Vec::new();
        for &b in bodies {
            out.extend(self.face_infos(b)?);
        }
        Ok(out)
    }
}

/// For push/pull: pairs each moved planar input face with the output face on the
/// same normal whose plane is offset by the distance.
fn moved_pairs(
    before: &[FaceInfo],
    after: &[FaceInfo],
    moved: &[(FaceId, f64)],
    tol: MatchTolerance,
) -> Vec<(FaceId, FaceId)> {
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut pairs = Vec::new();
    for &(id, distance) in moved {
        let Some(src) = before.iter().find(|f| f.id == id) else {
            continue;
        };
        let (SurfaceKind::Plane, Some(n)) = (src.surface, src.normal) else {
            continue;
        };
        let target = dot(n, src.centroid) + distance;
        let hit = after.iter().find(|f| {
            f.surface == SurfaceKind::Plane
                && f.normal
                    .is_some_and(|m| dot(m, n) >= (1.0 - tol.angular).min(1.0 - 1e-12))
                && (dot(n, f.centroid) - target).abs() <= tol.linear.max(1e-6 * distance.abs())
        });
        if let Some(f) = hit {
            pairs.push((id, f.id));
        }
    }
    pairs
}

impl<K: Kernel> Kernel for Guarded<K> {
    fn info(&self) -> KernelInfo {
        self.inner.info()
    }

    fn set_interrupt(&mut self, interrupt: Option<Interrupt>) {
        self.inner.set_interrupt(interrupt);
    }

    fn make_box(&mut self, spec: &BoxSpec) -> KernelResult<OpOutput> {
        let min = check_point("min", spec.min)?;
        let max = check_point("max", spec.max)?;
        for k in [0, 1, 2] {
            let (lo, hi) = (
                min.get(k).copied().unwrap_or(0.0),
                max.get(k).copied().unwrap_or(0.0),
            );
            check_length("max - min", hi - lo, true)?;
        }
        let spec = BoxSpec { min, max };
        let out = self.call_mut("make_box", |k| k.make_box(&spec))?;
        self.finish("make_box", out, Vec::new(), &[])
    }

    fn make_cylinder(&mut self, spec: &CylinderSpec) -> KernelResult<OpOutput> {
        let spec = CylinderSpec {
            base: check_point("base", spec.base)?,
            axis: check_direction("axis", spec.axis)?,
            radius: check_length("radius", spec.radius, true)?,
            height: check_length("height", spec.height, true)?,
        };
        let out = self.call_mut("make_cylinder", |k| k.make_cylinder(&spec))?;
        self.finish("make_cylinder", out, Vec::new(), &[])
    }

    fn boolean(&mut self, op: BooleanOp, a: BodyId, b: BodyId) -> KernelResult<OpOutput> {
        if a == b {
            return Err(KernelError::InvalidInput {
                param: "b",
                reason: "both operands are the same body".into(),
            });
        }
        let before = self.before(&[a, b])?;
        let out = self.call_mut("boolean", |k| k.boolean(op, a, b))?;
        self.finish("boolean", out, before, &[])
    }

    fn offset_faces(
        &mut self,
        body: BodyId,
        faces: &[FaceId],
        distance: f64,
    ) -> KernelResult<OpOutput> {
        let distance = check_length("distance", distance, false)?;
        if distance.abs() < MIN_LENGTH {
            return Err(KernelError::InvalidInput {
                param: "distance",
                reason: "zero offset".into(),
            });
        }
        if faces.is_empty() {
            return Err(KernelError::InvalidInput {
                param: "faces",
                reason: "no face selected".into(),
            });
        }
        let before = self.before(&[body])?;
        let moved: Vec<(FaceId, f64)> = faces.iter().map(|&f| (f, distance)).collect();
        let out = self.call_mut("offset_faces", |k| k.offset_faces(body, faces, distance))?;
        self.finish("offset_faces", out, before, &moved)
    }

    fn fillet(&mut self, body: BodyId, edges: &[EdgeId], radius: f64) -> KernelResult<OpOutput> {
        let radius = check_length("radius", radius, true)?;
        if edges.is_empty() {
            return Err(KernelError::InvalidInput {
                param: "edges",
                reason: "no edge selected".into(),
            });
        }
        let before = self.before(&[body])?;
        let out = self.call_mut("fillet", |k| k.fillet(body, edges, radius))?;
        self.finish("fillet", out, before, &[])
    }

    fn chamfer(&mut self, body: BodyId, edges: &[EdgeId], distance: f64) -> KernelResult<OpOutput> {
        let distance = check_length("distance", distance, true)?;
        if edges.is_empty() {
            return Err(KernelError::InvalidInput {
                param: "edges",
                reason: "no edge selected".into(),
            });
        }
        let before = self.before(&[body])?;
        let out = self.call_mut("chamfer", |k| k.chamfer(body, edges, distance))?;
        self.finish("chamfer", out, before, &[])
    }

    fn shell(
        &mut self,
        body: BodyId,
        openings: &[FaceId],
        thickness: f64,
    ) -> KernelResult<OpOutput> {
        let thickness = check_length("thickness", thickness, true)?;
        let before = self.before(&[body])?;
        let out = self.call_mut("shell", |k| k.shell(body, openings, thickness))?;
        self.finish("shell", out, before, &[])
    }

    fn extrude(&mut self, profile: &Profile, length: f64) -> KernelResult<OpOutput> {
        check_profile(profile)?;
        let length = check_length("length", length, true)?;
        let out = self.call_mut("extrude", |k| k.extrude(profile, length))?;
        self.finish("extrude", out, Vec::new(), &[])
    }

    fn delete_body(&mut self, body: BodyId) -> KernelResult<()> {
        self.call_mut("delete_body", |k| k.delete_body(body))
    }

    fn copy_body(&mut self, body: BodyId) -> KernelResult<OpOutput> {
        let out = self.call_mut("copy_body", |k| k.copy_body(body))?;
        self.finish("copy_body", out, Vec::new(), &[])
    }

    fn faces(&self, body: BodyId) -> KernelResult<Vec<FaceId>> {
        self.call_ref("faces", |k| k.faces(body))
    }

    fn edges(&self, body: BodyId) -> KernelResult<Vec<EdgeId>> {
        self.call_ref("edges", |k| k.edges(body))
    }

    fn face_info(&self, face: FaceId) -> KernelResult<FaceInfo> {
        self.call_ref("face_info", |k| k.face_info(face))
    }

    fn edge_endpoints(&self, edge: EdgeId) -> KernelResult<[[f64; 3]; 2]> {
        self.call_ref("edge_endpoints", |k| k.edge_endpoints(edge))
    }

    fn edge_faces(&self, body: BodyId, edge: EdgeId) -> KernelResult<[FaceId; 2]> {
        self.call_ref("edge_faces", |k| k.edge_faces(body, edge))
    }

    fn mass_properties(&self, body: BodyId) -> KernelResult<MassProperties> {
        self.call_ref("mass_properties", |k| k.mass_properties(body))
    }

    fn tessellate(&self, body: BodyId, chord: f64) -> KernelResult<TriMesh> {
        if !(chord.is_finite() && (CHORD_RANGE.0..=CHORD_RANGE.1).contains(&chord)) {
            return Err(KernelError::InvalidInput {
                param: "chord",
                reason: format!("{chord} mm outside {:?}", CHORD_RANGE),
            });
        }
        let mesh = self.call_ref("tessellate", |k| k.tessellate(body, chord))?;
        mesh.validate().map_err(|d| KernelError::InvalidResult {
            op: "tessellate",
            reason: format!("{d:?}"),
        })?;
        Ok(mesh)
    }

    fn check(&self, body: BodyId) -> KernelResult<Validity> {
        self.call_ref("check", |k| k.check(body))
    }

    fn export_step(&self, bodies: &[BodyId]) -> KernelResult<String> {
        self.call_ref("export_step", |k| k.export_step(bodies))
    }

    fn import_step(&mut self, text: &str) -> KernelResult<Vec<OpOutput>> {
        const MAX_STEP_BYTES: usize = 256 * 1024 * 1024;
        if text.len() > MAX_STEP_BYTES {
            return Err(KernelError::InvalidInput {
                param: "text",
                reason: "STEP file larger than 256 MiB".into(),
            });
        }
        let outs = self.call_mut("import_step", |k| k.import_step(text))?;
        let mut checked = Vec::with_capacity(outs.len());
        for out in outs {
            checked.push(self.finish("import_step", out, Vec::new(), &[])?);
        }
        Ok(checked)
    }
}
