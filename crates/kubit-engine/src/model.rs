//! The derived model: which kernel body each document body is now, and the
//! persistent name of every face and edge. Rebuilt by replaying the timeline.

use crate::error::CommandError;
use kubit_doc::{EdgeName, Feature, FeatureId, TopoName};
use kubit_kernel_api::{
    BodyId, BooleanOp, BoxSpec, CylinderSpec, EdgeId, EntityRef, FaceId, FaceInfo, Guarded, Kernel,
    KernelError, KernelResult, OpOutput, SurfaceKind, check_length, check_point,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A document body: created by one feature, edited by later ones.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyState {
    /// Current kernel body.
    pub kernel: BodyId,
    /// Persistent name → kernel face, for every face of the current body.
    pub names: BTreeMap<TopoName, FaceId>,
    /// Persistent name → kernel edge, for every edge whose name is unambiguous
    /// (ADR-0004). Derived from `names` after each rebuild of the body.
    pub edges: BTreeMap<EdgeName, EdgeId>,
    /// Last feature that changed this body.
    pub tip: FeatureId,
}

impl BodyState {
    /// Name of a kernel face of this body.
    pub fn name_of(&self, face: FaceId) -> Option<&TopoName> {
        self.names.iter().find(|(_, f)| **f == face).map(|(n, _)| n)
    }

    /// Name of a kernel edge of this body.
    pub fn edge_name_of(&self, edge: EdgeId) -> Option<&EdgeName> {
        self.edges.iter().find(|(_, e)| **e == edge).map(|(n, _)| n)
    }

    /// Names the edges of `kernel` from the face `names` and assembles the state.
    fn build<K: Kernel>(
        k: &Guarded<K>,
        kernel: BodyId,
        names: BTreeMap<TopoName, FaceId>,
        tip: FeatureId,
    ) -> KernelResult<Self> {
        let edges = name_edges(k, kernel, &names)?;
        Ok(Self {
            kernel,
            names,
            edges,
            tip,
        })
    }
}

/// Result of applying a feature on rebuild.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", content = "message", rename_all = "snake_case")]
pub enum FeatureStatus {
    /// Applied.
    Ok,
    /// Failed; later features that depend on it may fail too.
    Failed(String),
    /// Skipped because it is suppressed.
    Suppressed,
}

/// The derived model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    /// Bodies keyed by the feature that created them (shown as `F1`, …).
    pub bodies: BTreeMap<FeatureId, BodyState>,
    /// Status of every feature after the last rebuild.
    pub status: BTreeMap<FeatureId, FeatureStatus>,
}

impl Model {
    /// The body that owns a face name.
    pub fn body_of_face(&self, face: &TopoName) -> Option<(FeatureId, &BodyState)> {
        self.bodies
            .iter()
            .find(|(_, b)| b.names.contains_key(face))
            .map(|(k, b)| (*k, b))
    }

    /// The body that owns an edge name.
    pub fn body_of_edge(&self, edge: &EdgeName) -> Option<(FeatureId, &BodyState)> {
        self.bodies
            .iter()
            .find(|(_, b)| b.edges.contains_key(edge))
            .map(|(k, b)| (*k, b))
    }

    /// Every kernel body referenced by this model.
    pub fn kernel_bodies(&self) -> impl Iterator<Item = BodyId> + '_ {
        self.bodies.values().map(|b| b.kernel)
    }
}

/// `solid.box` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoxParams {
    /// Minimum corner, mm. Default: the origin.
    #[serde(default)]
    pub origin: [f64; 3],
    /// Size along X, Y, Z in mm; each > 0.
    pub size: [f64; 3],
}

/// `face.push_pull` parameters. Give `faces`, or the single-face form `face`
/// (kept for existing scripts and files); both may be combined.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PushPullParams {
    /// Persistent face name, e.g. `F1:+Z` (see `body.faces`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<String>,
    /// Persistent face names of one body, moved together by one operation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<String>,
    /// Distance along the outward normal, mm (negative pushes in).
    pub distance: f64,
}

impl PushPullParams {
    /// Every named face, `face` first, without repeats.
    pub fn all_faces(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for f in self.face.iter().chain(&self.faces) {
            if !out.contains(&f.as_str()) {
                out.push(f);
            }
        }
        out
    }
}

/// `solid.cylinder` parameters: a right circular cylinder along +Z.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CylinderParams {
    /// Centre of the base disc, mm. Default: the origin.
    #[serde(default)]
    pub origin: [f64; 3],
    /// Radius in mm; > 0.
    pub radius: f64,
    /// Height along +Z in mm; > 0.
    pub height: f64,
}

/// `edge.fillet` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FilletParams {
    /// Persistent edge names of one body, e.g. `F1:+X|F1:+Z` (see `selection.set`).
    pub edges: Vec<String>,
    /// Fillet radius in mm; > 0.
    pub radius: f64,
}

/// `edge.chamfer` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChamferParams {
    /// Persistent edge names of one body, e.g. `F1:+X|F1:+Z`.
    pub edges: Vec<String>,
    /// Equal chamfer distance in mm; > 0.
    pub distance: f64,
}

/// `body.shell` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShellParams {
    /// Faces of one body that are removed (open); at least one. Walls grow inwards.
    pub faces: Vec<String>,
    /// Wall thickness in mm; > 0.
    pub thickness: f64,
}

/// Kind of `body.boolean`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoolKind {
    /// target ∪ tool.
    Union,
    /// target − tool.
    Subtract,
    /// target ∩ tool.
    Intersect,
}

/// `body.boolean` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BooleanParams {
    /// Operation.
    pub op: BoolKind,
    /// Body that keeps its id, e.g. `F1` (the feature that created it).
    pub target: String,
    /// Body that is consumed, e.g. `F2`.
    pub tool: String,
}

/// Box face roles from the outward normal of a planar face.
fn box_role(info: &FaceInfo) -> Option<&'static str> {
    let n = info.normal?;
    let axes = [("X", n[0]), ("Y", n[1]), ("Z", n[2])];
    let (axis, v) = axes.iter().max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))?;
    if v.abs() < 0.999 {
        return None;
    }
    Some(match (*axis, *v > 0.0) {
        ("X", true) => "+X",
        ("X", false) => "-X",
        ("Y", true) => "+Y",
        ("Y", false) => "-Y",
        ("Z", true) => "+Z",
        _ => "-Z",
    })
}

/// Deterministic order for generated faces: by centroid, then area.
fn geometric_order(infos: &mut [FaceInfo]) {
    let key = |f: &FaceInfo| {
        // Quantise to 1 nm so tiny numeric noise does not reorder faces.
        #[allow(clippy::cast_possible_truncation)]
        let q = |x: f64| (x * 1e6).round() as i64;
        (
            q(f.centroid[0]),
            q(f.centroid[1]),
            q(f.centroid[2]),
            q(f.area),
        )
    };
    infos.sort_by_key(key);
}

fn face_infos<K: Kernel>(k: &Guarded<K>, body: BodyId) -> KernelResult<Vec<FaceInfo>> {
    k.faces(body)?.into_iter().map(|f| k.face_info(f)).collect()
}

/// Carries names from `old` through `out.history` to the faces of `out.body`.
fn propagate<K: Kernel>(
    k: &Guarded<K>,
    old: &BodyState,
    out: &OpOutput,
    feature: FeatureId,
) -> KernelResult<BTreeMap<TopoName, FaceId>> {
    let mut infos = face_infos(k, out.body)?;
    geometric_order(&mut infos);
    let output: Vec<FaceId> = infos.iter().map(|f| f.id).collect();
    let mut names: BTreeMap<TopoName, FaceId> = BTreeMap::new();
    for (name, face) in &old.names {
        if output.contains(face) {
            names.insert(name.clone(), *face);
            continue;
        }
        let images: Vec<FaceId> = out
            .history
            .images_of(EntityRef::Face(*face))
            .iter()
            .filter_map(|e| match e {
                EntityRef::Face(f) if output.contains(f) => Some(*f),
                _ => None,
            })
            .collect();
        match images.as_slice() {
            [] => {}
            [single] => {
                names.insert(name.clone(), *single);
            }
            many => {
                // Pieces in geometric order, so the numbering is reproducible.
                let ordered: Vec<FaceId> = output
                    .iter()
                    .copied()
                    .filter(|f| many.contains(f))
                    .collect();
                for (i, piece) in ordered.into_iter().enumerate() {
                    let ordinal = u32::try_from(i).unwrap_or(u32::MAX);
                    names.insert(
                        TopoName::Split {
                            parent: Box::new(name.clone()),
                            ordinal,
                        },
                        piece,
                    );
                }
            }
        }
    }
    let mut next = 0u32;
    for face in output {
        if !names.values().any(|f| *f == face) {
            names.insert(
                TopoName::Generated {
                    feature,
                    ordinal: next,
                },
                face,
            );
            next = next.saturating_add(1);
        }
    }
    Ok(names)
}

/// Names every edge of `body` by its two faces (ADR-0004). An edge stays
/// unnamed when a face is unnamed, when the backend cannot answer for it, or
/// when another edge of the body gets the same name (two faces that meet
/// twice). A backend without `edge_faces` names no edges. Panics, poisoning and
/// interrupts still fail the feature.
fn name_edges<K: Kernel>(
    k: &Guarded<K>,
    body: BodyId,
    faces: &BTreeMap<TopoName, FaceId>,
) -> KernelResult<BTreeMap<EdgeName, EdgeId>> {
    let by_face: BTreeMap<FaceId, &TopoName> = faces.iter().map(|(n, f)| (*f, n)).collect();
    // `None` marks a name claimed by more than one edge.
    let mut named: BTreeMap<EdgeName, Option<EdgeId>> = BTreeMap::new();
    for edge in k.edges(body)? {
        let [f, g] = match k.edge_faces(body, edge) {
            Ok(pair) => pair,
            Err(KernelError::Unsupported { .. }) => return Ok(BTreeMap::new()),
            Err(
                e @ (KernelError::Panicked { .. }
                | KernelError::Poisoned
                | KernelError::Interrupted { .. }),
            ) => return Err(e),
            Err(_) => continue,
        };
        let (Some(x), Some(y)) = (by_face.get(&f), by_face.get(&g)) else {
            continue;
        };
        named
            .entry(EdgeName::new((*x).clone(), (*y).clone()))
            .and_modify(|slot| *slot = None)
            .or_insert(Some(edge));
    }
    Ok(named
        .into_iter()
        .filter_map(|(name, edge)| Some((name, edge?)))
        .collect())
}

fn parse<T: for<'de> Deserialize<'de>>(
    command: &'static str,
    params: &serde_json::Value,
) -> Result<T, CommandError> {
    serde_json::from_value(params.clone()).map_err(|e| CommandError::InvalidParams {
        command,
        message: e.to_string(),
    })
}

fn invalid(command: &'static str, e: impl std::fmt::Display) -> CommandError {
    CommandError::InvalidParams {
        command,
        message: e.to_string(),
    }
}

/// Validates `solid.box` parameters (finite, bounded, positive size).
pub fn validate_box(p: &BoxParams) -> Result<BoxSpec, CommandError> {
    let origin = check_point("origin", p.origin).map_err(|e| invalid("solid.box", e))?;
    let mut max = origin;
    for (slot, (o, s)) in max.iter_mut().zip(origin.iter().zip(p.size)) {
        let s = check_length("size", s, true).map_err(|e| invalid("solid.box", e))?;
        *slot = o + s;
    }
    check_point("origin + size", max).map_err(|e| invalid("solid.box", e))?;
    Ok(BoxSpec { min: origin, max })
}

/// Resolves face names that must all belong to one body.
fn resolve_faces(
    model: &Model,
    command: &'static str,
    texts: &[&str],
) -> Result<(FeatureId, BodyState, Vec<FaceId>), CommandError> {
    if texts.is_empty() {
        return Err(invalid(command, "no face given"));
    }
    let mut owner: Option<(FeatureId, &BodyState)> = None;
    let mut faces = Vec::new();
    for text in texts {
        let name: TopoName = text.parse().map_err(|e| invalid(command, e))?;
        let (key, body) = model
            .body_of_face(&name)
            .ok_or_else(|| CommandError::Reference(format!("face `{name}`")))?;
        if owner.is_some_and(|(k, _)| k != key) {
            return Err(invalid(command, "faces belong to different bodies"));
        }
        owner = Some((key, body));
        let face = *body
            .names
            .get(&name)
            .ok_or_else(|| CommandError::Reference(format!("face `{name}`")))?;
        if !faces.contains(&face) {
            faces.push(face);
        }
    }
    let (key, body) = owner.ok_or_else(|| invalid(command, "no face given"))?;
    Ok((key, body.clone(), faces))
}

/// Resolves edge names that must all belong to one body.
fn resolve_edges(
    model: &Model,
    command: &'static str,
    texts: &[String],
) -> Result<(FeatureId, BodyState, Vec<EdgeId>), CommandError> {
    if texts.is_empty() {
        return Err(invalid(command, "no edge given"));
    }
    let mut owner: Option<(FeatureId, &BodyState)> = None;
    let mut edges = Vec::new();
    for text in texts {
        let name: EdgeName = text.parse().map_err(|e| invalid(command, e))?;
        let (key, body) = model
            .body_of_edge(&name)
            .ok_or_else(|| CommandError::Reference(format!("edge `{name}`")))?;
        if owner.is_some_and(|(k, _)| k != key) {
            return Err(invalid(command, "edges belong to different bodies"));
        }
        owner = Some((key, body));
        let edge = *body
            .edges
            .get(&name)
            .ok_or_else(|| CommandError::Reference(format!("edge `{name}`")))?;
        if !edges.contains(&edge) {
            edges.push(edge);
        }
    }
    let (key, body) = owner.ok_or_else(|| invalid(command, "no edge given"))?;
    Ok((key, body.clone(), edges))
}

/// Names the result of an operation on `old` and stores it under `key`.
fn replace_body<K: Kernel>(
    k: &Guarded<K>,
    model: &mut Model,
    key: FeatureId,
    old: &BodyState,
    out: &OpOutput,
    feature: FeatureId,
) -> Result<(), CommandError> {
    let names = propagate(k, old, out, feature)?;
    let state = BodyState::build(k, out.body, names, feature)?;
    model.bodies.insert(key, state);
    Ok(())
}

/// Applies one feature to the model. On error the model is left untouched
/// (results already created in the kernel are freed by the caller's GC).
pub fn apply_feature<K: Kernel>(
    k: &mut Guarded<K>,
    model: &mut Model,
    f: &Feature,
) -> Result<(), CommandError> {
    match f.command.as_str() {
        "solid.box" => {
            let p: BoxParams = parse("solid.box", &f.params)?;
            let spec = validate_box(&p)?;
            let out = k.make_box(&spec)?;
            let mut infos = face_infos(k, out.body)?;
            geometric_order(&mut infos);
            let mut names = BTreeMap::new();
            for info in &infos {
                let role = box_role(info).ok_or_else(|| {
                    CommandError::Reference(format!("box face {} is not axis-aligned", info.id.0))
                })?;
                names.insert(
                    TopoName::Primitive {
                        feature: f.id,
                        role: role.into(),
                    },
                    info.id,
                );
            }
            let state = BodyState::build(k, out.body, names, f.id)?;
            model.bodies.insert(f.id, state);
            Ok(())
        }
        "solid.cylinder" => {
            let p: CylinderParams = parse("solid.cylinder", &f.params)?;
            let spec = CylinderSpec {
                base: check_point("origin", p.origin).map_err(|e| invalid("solid.cylinder", e))?,
                axis: [0.0, 0.0, 1.0],
                radius: check_length("radius", p.radius, true)
                    .map_err(|e| invalid("solid.cylinder", e))?,
                height: check_length("height", p.height, true)
                    .map_err(|e| invalid("solid.cylinder", e))?,
            };
            check_point(
                "origin + height",
                [spec.base[0], spec.base[1], spec.base[2] + spec.height],
            )
            .map_err(|e| invalid("solid.cylinder", e))?;
            let out = k.make_cylinder(&spec)?;
            let mut infos = face_infos(k, out.body)?;
            geometric_order(&mut infos);
            let mut names = BTreeMap::new();
            let mut sides = 0u32;
            for info in &infos {
                let role = match (info.surface, box_role(info)) {
                    (SurfaceKind::Plane, Some(r @ ("+Z" | "-Z"))) => r.to_owned(),
                    (SurfaceKind::Plane, _) => {
                        return Err(CommandError::Reference(format!(
                            "cylinder face {} is not an end cap",
                            info.id.0
                        )));
                    }
                    _ => {
                        sides += 1;
                        if sides == 1 {
                            "side".to_owned()
                        } else {
                            format!("side{}", sides - 1)
                        }
                    }
                };
                names.insert(
                    TopoName::Primitive {
                        feature: f.id,
                        role,
                    },
                    info.id,
                );
            }
            let state = BodyState::build(k, out.body, names, f.id)?;
            model.bodies.insert(f.id, state);
            Ok(())
        }
        "face.push_pull" => {
            let p: PushPullParams = parse("face.push_pull", &f.params)?;
            let distance = check_length("distance", p.distance, false)
                .map_err(|e| invalid("face.push_pull", e))?;
            let texts = p.all_faces();
            let (key, old, faces) = resolve_faces(model, "face.push_pull", &texts)?;
            let out = k.offset_faces(old.kernel, &faces, distance)?;
            replace_body(k, model, key, &old, &out, f.id)
        }
        "edge.fillet" => {
            let p: FilletParams = parse("edge.fillet", &f.params)?;
            let radius =
                check_length("radius", p.radius, true).map_err(|e| invalid("edge.fillet", e))?;
            let (key, old, edges) = resolve_edges(model, "edge.fillet", &p.edges)?;
            let out = k.fillet(old.kernel, &edges, radius)?;
            replace_body(k, model, key, &old, &out, f.id)
        }
        "edge.chamfer" => {
            let p: ChamferParams = parse("edge.chamfer", &f.params)?;
            let distance = check_length("distance", p.distance, true)
                .map_err(|e| invalid("edge.chamfer", e))?;
            let (key, old, edges) = resolve_edges(model, "edge.chamfer", &p.edges)?;
            let out = k.chamfer(old.kernel, &edges, distance)?;
            replace_body(k, model, key, &old, &out, f.id)
        }
        "body.shell" => {
            let p: ShellParams = parse("body.shell", &f.params)?;
            let thickness = check_length("thickness", p.thickness, true)
                .map_err(|e| invalid("body.shell", e))?;
            let texts: Vec<&str> = p.faces.iter().map(String::as_str).collect();
            let (key, old, faces) = resolve_faces(model, "body.shell", &texts)?;
            let out = k.shell(old.kernel, &faces, thickness)?;
            replace_body(k, model, key, &old, &out, f.id)
        }
        "body.boolean" => {
            let p: BooleanParams = parse("body.boolean", &f.params)?;
            let body_ref = |text: &str| -> Result<FeatureId, CommandError> {
                let id: FeatureId = text.parse().map_err(|e| invalid("body.boolean", e))?;
                if model.bodies.contains_key(&id) {
                    Ok(id)
                } else {
                    Err(CommandError::Reference(format!("body {id}")))
                }
            };
            let (target, tool) = (body_ref(&p.target)?, body_ref(&p.tool)?);
            if target == tool {
                return Err(invalid("body.boolean", "target and tool are the same body"));
            }
            let (Some(a), Some(b)) = (model.bodies.get(&target), model.bodies.get(&tool)) else {
                return Err(CommandError::Reference("boolean operand".into()));
            };
            let op = match p.op {
                BoolKind::Union => BooleanOp::Union,
                BoolKind::Subtract => BooleanOp::Difference,
                BoolKind::Intersect => BooleanOp::Intersection,
            };
            let out = k.boolean(op, a.kernel, b.kernel)?;
            // Face names of both operands are unique (they embed feature ids), so
            // the union of the two maps is the history's source naming.
            let mut both = a.clone();
            both.names
                .extend(b.names.iter().map(|(n, f)| (n.clone(), *f)));
            let tool_body = tool;
            replace_body(k, model, target, &both, &out, f.id)?;
            model.bodies.remove(&tool_body);
            Ok(())
        }
        other => Err(CommandError::UnknownFeature {
            feature: f.id.to_string(),
            command: other.to_owned(),
        }),
    }
}

/// Whether a command id is a timeline feature (replayed on rebuild).
pub fn is_feature(command: &str) -> bool {
    matches!(
        command,
        "solid.box"
            | "solid.cylinder"
            | "face.push_pull"
            | "edge.fillet"
            | "edge.chamfer"
            | "body.shell"
            | "body.boolean"
    )
}
