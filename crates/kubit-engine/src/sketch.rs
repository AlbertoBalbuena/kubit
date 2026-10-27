//! Sketch features: a [`kubit_sketch::Sketch`] on a plane, stored in its feature's
//! params and solved on every rebuild, plus the commands that edit it in place.
//!
//! A sketch feature creates no body. Its plane is one of the world planes or a
//! planar face (re-resolved on rebuild, so a sketch follows the face it sits on).
//! Edits replace the feature's params and rebuild the document, so features that use
//! the sketch (extrusions of its regions) follow the change. Design: ADR-0005.

use crate::error::CommandError;
use crate::model::Model;
use crate::session::Session;
use crate::spec::CommandOutcome;
use kubit_doc::{FeatureId, TopoName};
use kubit_kernel_api::{Guarded, Kernel, Profile, ProfileLoop, ProfileSeg, SurfaceKind};
use kubit_sketch::{
    ConstraintId, ConstraintRecord, EntityId, EntityRecord, LoopShape, Region, Seg, Sketch,
    SolveOptions, SolveReport,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Where a sketch lies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PlaneRef {
    /// The world XY plane (u = +X, v = +Y, normal +Z).
    #[serde(rename = "XY")]
    Xy,
    /// The world XZ plane (u = +X, v = +Z, normal −Y).
    #[serde(rename = "XZ")]
    Xz,
    /// The world YZ plane (u = +Y, v = +Z, normal +X).
    #[serde(rename = "YZ")]
    Yz,
    /// A planar face, by persistent name (e.g. `F1:+Z`).
    #[serde(rename = "face")]
    Face(String),
}

/// A sketch plane in world space: origin, u axis and normal (v = normal × u).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PlaneFrame {
    /// Origin, mm.
    pub origin: [f64; 3],
    /// Unit u axis.
    pub x_axis: [f64; 3],
    /// Unit normal.
    pub normal: [f64; 3],
}

impl PlaneFrame {
    /// The unit v axis.
    pub fn y_axis(&self) -> [f64; 3] {
        cross(self.normal, self.x_axis)
    }

    /// World point of sketch point `(u, v)`.
    pub fn to_world(&self, p: [f64; 2]) -> [f64; 3] {
        let y = self.y_axis();
        [0, 1, 2].map(|k| {
            self.origin.get(k).copied().unwrap_or(0.0)
                + p[0] * self.x_axis.get(k).copied().unwrap_or(0.0)
                + p[1] * y.get(k).copied().unwrap_or(0.0)
        })
    }

    /// Sketch coordinates of a world point projected onto the plane.
    pub fn to_plane(&self, w: [f64; 3]) -> [f64; 2] {
        let d = [
            w[0] - self.origin[0],
            w[1] - self.origin[1],
            w[2] - self.origin[2],
        ];
        [dot(d, self.x_axis), dot(d, self.y_axis())]
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = dot(a, a).sqrt();
    (n.is_finite() && n > 1e-12).then(|| a.map(|c| c / n))
}

/// A solved sketch in the model.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SketchState {
    /// Its plane.
    pub frame: PlaneFrame,
    /// The solved sketch.
    pub sketch: Sketch,
    /// What the solve found (DOF, redundant and conflicting constraints).
    pub report: SolveReport,
}

impl SketchState {
    /// Closed regions, in sketch coordinates.
    pub fn regions(&self) -> Vec<Region> {
        kubit_sketch::profiles(&self.sketch)
    }
}

/// `sketch.create` parameters (and the stored feature).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SketchParams {
    /// `"XY"`, `"XZ"`, `"YZ"` or `{"face": "F1:+Z"}`.
    pub plane: PlaneRef,
    /// The sketch content (empty when created).
    #[serde(default)]
    #[schemars(with = "serde_json::Value")]
    pub sketch: Sketch,
}

fn invalid(command: &'static str, e: impl std::fmt::Display) -> CommandError {
    CommandError::InvalidParams {
        command,
        message: e.to_string(),
    }
}

/// Resolves a plane in the current model.
fn frame_of<K: Kernel>(
    k: &Guarded<K>,
    model: &Model,
    plane: &PlaneRef,
) -> Result<PlaneFrame, CommandError> {
    Ok(match plane {
        PlaneRef::Xy => PlaneFrame {
            origin: [0.0; 3],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        },
        PlaneRef::Xz => PlaneFrame {
            origin: [0.0; 3],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, -1.0, 0.0],
        },
        PlaneRef::Yz => PlaneFrame {
            origin: [0.0; 3],
            x_axis: [0.0, 1.0, 0.0],
            normal: [1.0, 0.0, 0.0],
        },
        PlaneRef::Face(text) => {
            let name: TopoName = text.parse().map_err(|e| invalid("sketch.create", e))?;
            let (_, body) = model
                .body_of_face(&name)
                .ok_or_else(|| CommandError::Reference(format!("face {name}")))?;
            let face = *body
                .names
                .get(&name)
                .ok_or_else(|| CommandError::Reference(format!("face {name}")))?;
            let info = k.face_info(face)?;
            let normal = match (info.surface, info.normal) {
                (SurfaceKind::Plane, Some(n)) => unit(n),
                _ => None,
            }
            .ok_or_else(|| invalid("sketch.create", format!("face {name} is not planar")))?;
            // u: world X projected onto the plane (world Y when X is the normal).
            let project = |a: [f64; 3]| {
                let d = dot(a, normal);
                unit([
                    a[0] - d * normal[0],
                    a[1] - d * normal[1],
                    a[2] - d * normal[2],
                ])
            };
            let x_axis = project([1.0, 0.0, 0.0])
                .filter(|_| dot([1.0, 0.0, 0.0], normal).abs() < 0.999)
                .or_else(|| project([0.0, 1.0, 0.0]))
                .ok_or_else(|| invalid("sketch.create", "degenerate face plane"))?;
            // Origin: the world origin projected onto the plane, so the frame stays
            // put when the face grows or shrinks.
            let h = dot(info.centroid, normal);
            PlaneFrame {
                origin: normal.map(|c| c * h),
                x_axis,
                normal,
            }
        }
    })
}

/// Applies a `sketch.create` feature: resolves its plane and solves it.
pub fn apply<K: Kernel>(
    k: &Guarded<K>,
    model: &mut Model,
    id: FeatureId,
    params: &serde_json::Value,
) -> Result<(), CommandError> {
    let p: SketchParams =
        serde_json::from_value(params.clone()).map_err(|e| invalid("sketch.create", e))?;
    let frame = frame_of(k, model, &p.plane)?;
    let mut sketch = p.sketch;
    let report = kubit_sketch::solve(&mut sketch, &SolveOptions::default())
        .map_err(|e| invalid("sketch.create", e))?;
    model.sketches.insert(
        id,
        SketchState {
            frame,
            sketch,
            report,
        },
    );
    Ok(())
}

/// The kernel profile of region `region` of sketch `sketch`.
pub fn profile_of(model: &Model, sketch: FeatureId, region: u32) -> Result<Profile, CommandError> {
    let state = model
        .sketches
        .get(&sketch)
        .ok_or_else(|| CommandError::Reference(format!("sketch {sketch}")))?;
    let r = state
        .regions()
        .into_iter()
        .find(|r| r.id.0 == region)
        .ok_or_else(|| CommandError::Reference(format!("region {region} of sketch {sketch}")))?;
    let convert = |shape: &LoopShape| match shape {
        LoopShape::Circle { center, radius } => ProfileLoop::Circle {
            center: *center,
            radius: *radius,
        },
        LoopShape::Segments { start, segments } => ProfileLoop::Segments {
            start: *start,
            segments: segments
                .iter()
                .map(|s| match *s {
                    Seg::LineTo(p) => ProfileSeg::LineTo(p),
                    Seg::ArcTo { to, via } => ProfileSeg::ArcTo { to, via },
                })
                .collect(),
        },
    };
    Ok(Profile {
        origin: state.frame.origin,
        x_axis: state.frame.x_axis,
        normal: state.frame.normal,
        outer: convert(&r.outer.shape),
        holes: r.holes.iter().map(|h| convert(&h.shape)).collect(),
    })
}

// --- commands --------------------------------------------------------------------

/// `sketch.add` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddParams {
    /// The sketch feature, e.g. `F2`.
    pub sketch: String,
    /// Id the first new entity gets (the sketch's `next_id`); new entities and
    /// constraints are numbered from it in order, so they can reference each other.
    pub first_id: u32,
    /// Entities to add, in order.
    #[serde(default)]
    #[schemars(with = "Vec<serde_json::Value>")]
    pub entities: Vec<EntityRecord>,
    /// Constraints to add after the entities.
    #[serde(default)]
    #[schemars(with = "Vec<serde_json::Value>")]
    pub constraints: Vec<ConstraintRecord>,
}

/// `sketch.set_dimension` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DimensionParams {
    /// The sketch feature.
    pub sketch: String,
    /// Constraint id.
    pub constraint: u32,
    /// New value: mm, or radians for angles.
    pub value: f64,
}

/// `sketch.delete` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteParams {
    /// The sketch feature.
    pub sketch: String,
    /// Entities to remove (with their constraints and orphan points).
    #[serde(default)]
    pub entities: Vec<u32>,
    /// Constraints to remove.
    #[serde(default)]
    pub constraints: Vec<u32>,
}

/// `sketch.move_point` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveParams {
    /// The sketch feature.
    pub sketch: String,
    /// The point.
    pub point: u32,
    /// Where to drag it, sketch coordinates in mm.
    pub to: [f64; 2],
}

fn feature_id(command: &'static str, text: &str) -> Result<FeatureId, CommandError> {
    text.parse().map_err(|e| invalid(command, e))
}

/// The stored params of sketch feature `id`.
fn stored(s: &Session, command: &'static str, id: FeatureId) -> Result<SketchParams, CommandError> {
    let f = s
        .doc()
        .feature(id)
        .filter(|f| f.command == "sketch.create")
        .ok_or_else(|| CommandError::Reference(format!("sketch {id}")))?;
    serde_json::from_value(f.params.clone()).map_err(|e| invalid(command, e))
}

/// Stores an edited sketch (solved first, so the document keeps solved positions).
fn store(
    s: &mut Session,
    command: &'static str,
    id: FeatureId,
    mut p: SketchParams,
) -> Result<CommandOutcome, CommandError> {
    let report = kubit_sketch::solve(&mut p.sketch, &SolveOptions::default())
        .map_err(|e| invalid(command, e))?;
    let params = serde_json::to_value(&p).map_err(|e| CommandError::Format(e.to_string()))?;
    s.edit_feature(id, params)?;
    Ok(CommandOutcome {
        summary: format!(
            "Sketch {id}: {} DOF{}{}",
            report.dof,
            if report.redundant.is_empty() {
                String::new()
            } else {
                format!(", {} redundant", report.redundant.len())
            },
            if report.conflicting.is_empty() {
                String::new()
            } else {
                format!(", {} conflicting", report.conflicting.len())
            },
        ),
        data: json!({ "sketch": id.to_string(), "report": report }),
    })
}

pub(crate) fn run_create(
    s: &mut Session,
    v: serde_json::Value,
) -> Result<CommandOutcome, CommandError> {
    let mut p: SketchParams = serde_json::from_value(v).map_err(|e| invalid("sketch.create", e))?;
    kubit_sketch::solve(&mut p.sketch, &SolveOptions::default())
        .map_err(|e| invalid("sketch.create", e))?;
    let canonical = serde_json::to_value(&p).map_err(|e| CommandError::Format(e.to_string()))?;
    let id = s.add_feature("sketch.create", canonical)?;
    Ok(CommandOutcome {
        summary: format!("Created sketch {id}"),
        data: json!({ "feature": id.to_string(), "sketch": id.to_string() }),
    })
}

pub(crate) fn run_add(
    s: &mut Session,
    v: serde_json::Value,
) -> Result<CommandOutcome, CommandError> {
    const C: &str = "sketch.add";
    let p: AddParams = serde_json::from_value(v).map_err(|e| invalid(C, e))?;
    let id = feature_id(C, &p.sketch)?;
    let mut stored = stored(s, C, id)?;
    if stored.sketch.next_id != p.first_id {
        return Err(invalid(
            C,
            format!(
                "first_id is {}, the sketch's next id is {}",
                p.first_id, stored.sketch.next_id
            ),
        ));
    }
    let mut added = Vec::new();
    for e in &p.entities {
        let eid: EntityId = stored
            .sketch
            .add_entity(e.entity, e.construction)
            .map_err(|e| invalid(C, e))?;
        added.push(eid.0);
    }
    let mut constraints = Vec::new();
    for c in &p.constraints {
        let cid: ConstraintId = stored
            .sketch
            .add_constraint(c.constraint, c.reference)
            .map_err(|e| invalid(C, e))?;
        constraints.push(cid.0);
    }
    let mut out = store(s, C, id, stored)?;
    if let Some(obj) = out.data.as_object_mut() {
        obj.insert("entities".into(), json!(added));
        obj.insert("constraints".into(), json!(constraints));
    }
    Ok(out)
}

pub(crate) fn run_set_dimension(
    s: &mut Session,
    v: serde_json::Value,
) -> Result<CommandOutcome, CommandError> {
    const C: &str = "sketch.set_dimension";
    let p: DimensionParams = serde_json::from_value(v).map_err(|e| invalid(C, e))?;
    let id = feature_id(C, &p.sketch)?;
    let mut stored = stored(s, C, id)?;
    stored
        .sketch
        .set_dimension(ConstraintId(p.constraint), p.value)
        .map_err(|e| invalid(C, e))?;
    store(s, C, id, stored)
}

pub(crate) fn run_delete(
    s: &mut Session,
    v: serde_json::Value,
) -> Result<CommandOutcome, CommandError> {
    const C: &str = "sketch.delete";
    let p: DeleteParams = serde_json::from_value(v).map_err(|e| invalid(C, e))?;
    let id = feature_id(C, &p.sketch)?;
    let mut stored = stored(s, C, id)?;
    for c in &p.constraints {
        stored
            .sketch
            .remove_constraint(ConstraintId(*c))
            .map_err(|e| invalid(C, e))?;
    }
    // Curves before points, so a point is not refused for still being used.
    let mut ids: Vec<EntityId> = p.entities.iter().map(|e| EntityId(*e)).collect();
    ids.sort_by_key(|e| {
        matches!(
            stored.sketch.entity(*e),
            Ok(kubit_sketch::Entity::Point { .. })
        )
    });
    for e in ids {
        if stored.sketch.entities.contains_key(&e) {
            stored
                .sketch
                .remove_entity(e)
                .map_err(|err| invalid(C, err))?;
        }
    }
    store(s, C, id, stored)
}

pub(crate) fn run_move_point(
    s: &mut Session,
    v: serde_json::Value,
) -> Result<CommandOutcome, CommandError> {
    const C: &str = "sketch.move_point";
    let p: MoveParams = serde_json::from_value(v).map_err(|e| invalid(C, e))?;
    let id = feature_id(C, &p.sketch)?;
    let mut stored = stored(s, C, id)?;
    kubit_sketch::drag(
        &mut stored.sketch,
        EntityId(p.point),
        p.to,
        &SolveOptions::default(),
    )
    .map_err(|e| invalid(C, e))?;
    store(s, C, id, stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_planes_are_right_handed_and_round_trip() {
        for (frame, v) in [
            (
                PlaneFrame {
                    origin: [0.0; 3],
                    x_axis: [1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                },
                [0.0, 1.0, 0.0],
            ),
            (
                PlaneFrame {
                    origin: [0.0; 3],
                    x_axis: [1.0, 0.0, 0.0],
                    normal: [0.0, -1.0, 0.0],
                },
                [0.0, 0.0, 1.0],
            ),
            (
                PlaneFrame {
                    origin: [0.0; 3],
                    x_axis: [0.0, 1.0, 0.0],
                    normal: [1.0, 0.0, 0.0],
                },
                [0.0, 0.0, 1.0],
            ),
        ] {
            assert_eq!(frame.y_axis(), v);
            let w = frame.to_world([3.0, -2.0]);
            let back = frame.to_plane(w);
            assert!((back[0] - 3.0).abs() < 1e-12 && (back[1] + 2.0).abs() < 1e-12);
        }
    }
}
