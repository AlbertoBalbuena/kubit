//! `OpHistory`: what an operation did to the topology, and its reconstruction by
//! geometric matching for backends that do not report it.

use crate::types::{EntityRef, FaceId, FaceInfo, SurfaceKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Where a history record came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistorySource {
    /// Reported by the backend.
    Native,
    /// Rebuilt by the facade from face geometry before and after.
    #[default]
    Reconstructed,
}

/// Generated / modified / deleted entities of one operation.
///
/// Entities that survive unchanged with the same id are not listed. All maps are
/// ordered (`BTreeMap`) so the record is deterministic and diffable.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpHistory {
    /// New entities, keyed by the input entity (or `None` for a primitive) they come from.
    pub generated: BTreeMap<Option<EntityRef>, Vec<EntityRef>>,
    /// Input entity → the output entities that are its pieces or its moved version.
    pub modified: BTreeMap<EntityRef, Vec<EntityRef>>,
    /// Input entities with no image in the output.
    pub deleted: BTreeSet<EntityRef>,
    /// Which part of a profile sweep each generated face is, when the backend knows.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub face_roles: BTreeMap<FaceId, FaceRole>,
    /// Who produced the record.
    pub source: HistorySource,
}

/// The part of an extruded profile a generated face stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FaceRole {
    /// The cap on the profile plane (its outward normal opposes the extrusion).
    CapMinus,
    /// The cap the extrusion ends on (outward normal along the extrusion).
    CapPlus,
    /// The wall swept by one segment; indices as the profile was written
    /// (loop 0 is the outer, holes follow).
    Side {
        /// Loop index.
        loop_index: usize,
        /// Segment index in that loop (a circle has segment 0).
        segment: usize,
    },
}

impl OpHistory {
    /// An empty native record.
    pub fn native() -> Self {
        Self {
            source: HistorySource::Native,
            ..Self::default()
        }
    }

    /// Records `output` as generated from `origin`.
    pub fn add_generated(&mut self, origin: Option<EntityRef>, output: EntityRef) {
        self.generated.entry(origin).or_default().push(output);
    }

    /// Records `output` as a modified image of `input`.
    pub fn add_modified(&mut self, input: EntityRef, output: EntityRef) {
        self.modified.entry(input).or_default().push(output);
    }

    /// Total counts `(generated, modified, deleted)`.
    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.generated.values().map(Vec::len).sum(),
            self.modified.values().map(Vec::len).sum(),
            self.deleted.len(),
        )
    }

    /// All outputs that came from `input` (modified images), in order.
    pub fn images_of(&self, input: EntityRef) -> &[EntityRef] {
        self.modified.get(&input).map_or(&[], Vec::as_slice)
    }
}

/// Matching tolerances for reconstruction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchTolerance {
    /// Max distance between planes / centroids, mm.
    pub linear: f64,
    /// Max angle between normals, radians.
    pub angular: f64,
}

impl Default for MatchTolerance {
    fn default() -> Self {
        Self {
            linear: 1e-4,
            angular: 1e-6,
        }
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    dot(d, d).sqrt()
}

/// Whether two faces lie on the same carrier surface (as far as `FaceInfo` tells).
fn same_carrier(a: &FaceInfo, b: &FaceInfo, tol: MatchTolerance) -> bool {
    if a.surface != b.surface {
        return false;
    }
    match (a.surface, a.normal, b.normal) {
        (SurfaceKind::Plane, Some(na), Some(nb)) => {
            let cos = dot(na, nb).clamp(-1.0, 1.0);
            // Same orientation, same plane offset.
            cos.acos() <= tol.angular
                && (dot(na, a.centroid) - dot(nb, b.centroid)).abs() <= tol.linear
        }
        // Without axis data, a curved face matches when it overlaps in place:
        // the centroid of the smaller lies close to the bigger one's (scaled by size).
        _ => {
            let size = a.area.max(b.area).max(0.0).sqrt();
            dist(a.centroid, b.centroid) <= tol.linear.max(0.5 * size)
        }
    }
}

/// Rebuilds face history from the faces of the inputs and of the output.
///
/// - an output face whose id is an input id → unchanged (not recorded);
/// - `moved` lists faces the operation moved on purpose (push/pull): input → output by order;
/// - otherwise an output face on the same carrier as an input face → `modified`;
/// - remaining output faces → `generated` (origin `None`);
/// - input faces with no image → `deleted`.
///
/// Deterministic: inputs are visited in id order and ties go to the lowest id.
pub fn reconstruct_faces(
    before: &[FaceInfo],
    after: &[FaceInfo],
    moved: &[(FaceId, FaceId)],
    tol: MatchTolerance,
) -> OpHistory {
    let mut h = OpHistory::default();
    let before_ids: BTreeSet<FaceId> = before.iter().map(|f| f.id).collect();
    let mut imaged: BTreeSet<FaceId> = BTreeSet::new();
    let mut claimed: BTreeSet<FaceId> = BTreeSet::new();

    for f in after.iter().filter(|f| before_ids.contains(&f.id)) {
        imaged.insert(f.id);
        claimed.insert(f.id);
    }
    for &(input, output) in moved {
        if after.iter().any(|f| f.id == output) && !claimed.contains(&output) {
            h.add_modified(EntityRef::Face(input), EntityRef::Face(output));
            imaged.insert(input);
            claimed.insert(output);
        }
    }
    let mut sorted_before: Vec<&FaceInfo> = before.iter().collect();
    sorted_before.sort_by_key(|f| f.id);
    let mut sorted_after: Vec<&FaceInfo> = after.iter().collect();
    sorted_after.sort_by_key(|f| f.id);
    for out in sorted_after.iter().filter(|f| !claimed.contains(&f.id)) {
        let source = sorted_before
            .iter()
            .filter(|b| !moved.iter().any(|&(i, _)| i == b.id))
            .find(|b| same_carrier(b, out, tol));
        match source {
            Some(b) => {
                h.add_modified(EntityRef::Face(b.id), EntityRef::Face(out.id));
                imaged.insert(b.id);
            }
            None => h.add_generated(None, EntityRef::Face(out.id)),
        }
    }
    for b in &sorted_before {
        if !imaged.contains(&b.id) {
            h.deleted.insert(EntityRef::Face(b.id));
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(id: u64, n: [f64; 3], c: [f64; 3], area: f64) -> FaceInfo {
        FaceInfo {
            id: FaceId(id),
            surface: SurfaceKind::Plane,
            normal: Some(n),
            centroid: c,
            area,
        }
    }

    /// Box 10×10×10 with faces 0..6.
    fn cube(base: u64, top_z: f64) -> Vec<FaceInfo> {
        vec![
            plane(base, [0.0, 0.0, -1.0], [5.0, 5.0, 0.0], 100.0),
            plane(base + 1, [0.0, 0.0, 1.0], [5.0, 5.0, top_z], 100.0),
            plane(
                base + 2,
                [-1.0, 0.0, 0.0],
                [0.0, 5.0, top_z / 2.0],
                10.0 * top_z,
            ),
            plane(
                base + 3,
                [1.0, 0.0, 0.0],
                [10.0, 5.0, top_z / 2.0],
                10.0 * top_z,
            ),
            plane(
                base + 4,
                [0.0, -1.0, 0.0],
                [5.0, 0.0, top_z / 2.0],
                10.0 * top_z,
            ),
            plane(
                base + 5,
                [0.0, 1.0, 0.0],
                [5.0, 10.0, top_z / 2.0],
                10.0 * top_z,
            ),
        ]
    }

    #[test]
    fn push_pull_with_new_ids_maps_every_face() {
        // A backend that renumbers everything: the top moved (declared), sides grew.
        let h = reconstruct_faces(
            &cube(0, 10.0),
            &cube(100, 20.0),
            &[(FaceId(1), FaceId(101))],
            MatchTolerance::default(),
        );
        assert_eq!(h.counts(), (0, 6, 0));
        assert_eq!(
            h.images_of(EntityRef::Face(FaceId(1))),
            &[EntityRef::Face(FaceId(101))]
        );
        assert_eq!(
            h.images_of(EntityRef::Face(FaceId(0))),
            &[EntityRef::Face(FaceId(100))]
        );
    }

    #[test]
    fn surviving_ids_are_not_recorded() {
        let h = reconstruct_faces(
            &cube(0, 10.0),
            &cube(0, 10.0),
            &[],
            MatchTolerance::default(),
        );
        assert_eq!(h.counts(), (0, 0, 0));
    }

    #[test]
    fn hole_generates_a_face_and_keeps_split_faces_modified() {
        let before = cube(0, 10.0);
        let mut after = cube(10, 10.0);
        after.push(FaceInfo {
            id: FaceId(99),
            surface: SurfaceKind::Cylinder,
            normal: None,
            centroid: [5.0, 5.0, 5.0],
            area: 62.8,
        });
        let h = reconstruct_faces(&before, &after, &[], MatchTolerance::default());
        assert_eq!(h.counts(), (1, 6, 0));
        assert_eq!(h.generated.get(&None).map(Vec::len), Some(1));
    }

    #[test]
    fn removed_face_is_deleted() {
        let before = cube(0, 10.0);
        let after: Vec<FaceInfo> = cube(0, 10.0)
            .into_iter()
            .filter(|f| f.id != FaceId(1))
            .collect();
        let h = reconstruct_faces(&before, &after, &[], MatchTolerance::default());
        assert!(h.deleted.contains(&EntityRef::Face(FaceId(1))));
    }

    #[test]
    fn reconstruction_is_deterministic() {
        let a = reconstruct_faces(
            &cube(0, 10.0),
            &cube(50, 12.0),
            &[],
            MatchTolerance::default(),
        );
        let mut shuffled = cube(50, 12.0);
        shuffled.reverse();
        let b = reconstruct_faces(&cube(0, 10.0), &shuffled, &[], MatchTolerance::default());
        assert_eq!(a, b);
    }
}
