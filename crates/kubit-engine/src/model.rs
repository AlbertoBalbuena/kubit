//! The derived model: which kernel body each document body is now, and the
//! persistent name of every face. Rebuilt by replaying the timeline.

use crate::error::CommandError;
use kubit_doc::{Feature, FeatureId, TopoName};
use kubit_kernel_api::{
    BodyId, BoxSpec, EntityRef, FaceId, FaceInfo, Guarded, Kernel, KernelResult, OpOutput,
    check_length, check_point,
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
    /// Last feature that changed this body.
    pub tip: FeatureId,
}

impl BodyState {
    /// Name of a kernel face of this body.
    pub fn name_of(&self, face: FaceId) -> Option<&TopoName> {
        self.names.iter().find(|(_, f)| **f == face).map(|(n, _)| n)
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

/// `face.push_pull` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PushPullParams {
    /// Persistent face name, e.g. `F1:+Z` (see `body.faces`).
    pub face: String,
    /// Distance along the outward normal, mm (negative pushes in).
    pub distance: f64,
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
            model.bodies.insert(
                f.id,
                BodyState {
                    kernel: out.body,
                    names,
                    tip: f.id,
                },
            );
            Ok(())
        }
        "face.push_pull" => {
            let p: PushPullParams = parse("face.push_pull", &f.params)?;
            let name: TopoName = p.face.parse().map_err(|e| invalid("face.push_pull", e))?;
            let distance = check_length("distance", p.distance, false)
                .map_err(|e| invalid("face.push_pull", e))?;
            let (key, body) = model
                .body_of_face(&name)
                .ok_or_else(|| CommandError::Reference(format!("face `{name}`")))?;
            let face = *body
                .names
                .get(&name)
                .ok_or_else(|| CommandError::Reference(format!("face `{name}`")))?;
            let old = body.clone();
            let out = k.offset_faces(old.kernel, &[face], distance)?;
            let names = propagate(k, &old, &out, f.id)?;
            model.bodies.insert(
                key,
                BodyState {
                    kernel: out.body,
                    names,
                    tip: f.id,
                },
            );
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
    matches!(command, "solid.box" | "face.push_pull")
}
