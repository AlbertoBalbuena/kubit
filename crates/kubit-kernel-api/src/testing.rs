//! `BoxKernel`: an exact reference kernel for axis-aligned boxes.
//!
//! It exists to test the facade, the engine and the UI without a real B-rep
//! library, and as an oracle: every result has closed-form volume and area. It
//! supports `make_box`, `offset_faces` (a box face moved along its normal stays a
//! box), copies, queries, tessellation and its own checker. Everything else is
//! `Unsupported`. Enabled with the `testing` feature.

use crate::error::{KernelError, KernelResult};
use crate::history::OpHistory;
use crate::kernel::{Kernel, OpOutput};
use crate::types::{
    BodyId, BoxSpec, Capabilities, EdgeId, EntityRef, FaceId, FaceInfo, KernelInfo, MassProperties,
    SurfaceKind, Validity,
};
use kubit_mesh::{FaceTag, TriMesh};
use std::collections::BTreeMap;

/// Ids are packed as `body * 64 + local` so they are stable and decodable.
const STRIDE: u64 = 64;

/// The six faces of a box: (axis, max side). Order: −X, +X, −Y, +Y, −Z, +Z.
const FACES: [(usize, bool); 6] = [
    (0, false),
    (0, true),
    (1, false),
    (1, true),
    (2, false),
    (2, true),
];

/// The twelve edges of a box as (along axis, side on the next axis, side on the one after).
const EDGES: [(usize, bool, bool); 12] = [
    (0, false, false),
    (0, true, false),
    (0, false, true),
    (0, true, true),
    (1, false, false),
    (1, true, false),
    (1, false, true),
    (1, true, true),
    (2, false, false),
    (2, true, false),
    (2, false, true),
    (2, true, true),
];

/// Exact kernel for axis-aligned boxes.
#[derive(Debug, Default)]
pub struct BoxKernel {
    bodies: BTreeMap<u64, BoxSpec>,
    next: u64,
}

fn get(spec: &BoxSpec, side: bool, axis: usize) -> f64 {
    let src = if side { spec.max } else { spec.min };
    src.get(axis).copied().unwrap_or(0.0)
}

impl BoxKernel {
    /// A kernel with no bodies.
    pub fn new() -> Self {
        Self::default()
    }

    fn body(&self, id: BodyId) -> KernelResult<&BoxSpec> {
        self.bodies.get(&id.0).ok_or(KernelError::NotFound {
            kind: "body",
            id: id.0,
        })
    }

    fn insert(&mut self, spec: BoxSpec) -> BodyId {
        self.next += 1;
        self.bodies.insert(self.next, spec);
        BodyId(self.next)
    }

    fn face_id(body: BodyId, local: usize) -> FaceId {
        FaceId(body.0 * STRIDE + local as u64)
    }

    fn edge_id(body: BodyId, local: usize) -> EdgeId {
        EdgeId(body.0 * STRIDE + 6 + local as u64)
    }

    fn decode(id: u64) -> (BodyId, usize) {
        (
            BodyId(id / STRIDE),
            usize::try_from(id % STRIDE).unwrap_or(usize::MAX),
        )
    }

    fn primitive_history(body: BodyId) -> OpHistory {
        let mut h = OpHistory::native();
        h.add_generated(None, EntityRef::Body(body));
        for i in 0..6 {
            h.add_generated(None, EntityRef::Face(Self::face_id(body, i)));
        }
        for i in 0..12 {
            h.add_generated(None, EntityRef::Edge(Self::edge_id(body, i)));
        }
        h
    }

    fn corners(spec: &BoxSpec) -> [[f64; 3]; 8] {
        let mut out = [[0.0; 3]; 8];
        for (i, c) in out.iter_mut().enumerate() {
            *c = [
                get(spec, i & 1 != 0, 0),
                get(spec, i & 2 != 0, 1),
                get(spec, i & 4 != 0, 2),
            ];
        }
        out
    }
}

impl Kernel for BoxKernel {
    fn info(&self) -> KernelInfo {
        KernelInfo {
            name: "box-reference".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            capabilities: Capabilities {
                offset_faces: true,
                native_history: true,
                exact_mass: true,
                ..Capabilities::default()
            },
        }
    }

    fn make_box(&mut self, spec: &BoxSpec) -> KernelResult<OpOutput> {
        let body = self.insert(*spec);
        Ok(OpOutput {
            body,
            history: Self::primitive_history(body),
        })
    }

    fn offset_faces(
        &mut self,
        body: BodyId,
        faces: &[FaceId],
        distance: f64,
    ) -> KernelResult<OpOutput> {
        let mut spec = *self.body(body)?;
        for f in faces {
            let (owner, local) = Self::decode(f.0);
            let &(axis, side) =
                FACES
                    .get(local)
                    .filter(|_| owner == body)
                    .ok_or(KernelError::NotFound {
                        kind: "face",
                        id: f.0,
                    })?;
            let slot = if side {
                spec.max.get_mut(axis)
            } else {
                spec.min.get_mut(axis)
            };
            if let Some(v) = slot {
                *v += if side { distance } else { -distance };
            }
        }
        for axis in 0..3 {
            if get(&spec, true, axis) - get(&spec, false, axis) <= 0.0 {
                return Err(KernelError::Failed {
                    op: "offset_faces",
                    message: "the box would collapse".into(),
                });
            }
        }
        let out = self.insert(spec);
        // Every input entity maps one-to-one onto the same local slot of the new body.
        let mut h = OpHistory::native();
        h.add_modified(EntityRef::Body(body), EntityRef::Body(out));
        for i in 0..6 {
            h.add_modified(
                EntityRef::Face(Self::face_id(body, i)),
                EntityRef::Face(Self::face_id(out, i)),
            );
        }
        for i in 0..12 {
            h.add_modified(
                EntityRef::Edge(Self::edge_id(body, i)),
                EntityRef::Edge(Self::edge_id(out, i)),
            );
        }
        Ok(OpOutput {
            body: out,
            history: h,
        })
    }

    fn delete_body(&mut self, body: BodyId) -> KernelResult<()> {
        self.bodies
            .remove(&body.0)
            .map(|_| ())
            .ok_or(KernelError::NotFound {
                kind: "body",
                id: body.0,
            })
    }

    fn copy_body(&mut self, body: BodyId) -> KernelResult<OpOutput> {
        let spec = *self.body(body)?;
        let out = self.insert(spec);
        Ok(OpOutput {
            body: out,
            history: Self::primitive_history(out),
        })
    }

    fn faces(&self, body: BodyId) -> KernelResult<Vec<FaceId>> {
        self.body(body)?;
        Ok((0..6).map(|i| Self::face_id(body, i)).collect())
    }

    fn edges(&self, body: BodyId) -> KernelResult<Vec<EdgeId>> {
        self.body(body)?;
        Ok((0..12).map(|i| Self::edge_id(body, i)).collect())
    }

    fn face_info(&self, face: FaceId) -> KernelResult<FaceInfo> {
        let (body, local) = Self::decode(face.0);
        let spec = self.body(body)?;
        let &(axis, side) = FACES.get(local).ok_or(KernelError::NotFound {
            kind: "face",
            id: face.0,
        })?;
        let mut normal = [0.0; 3];
        let mut centroid = [0.0; 3];
        let mut area = 1.0;
        for k in 0..3 {
            let (lo, hi) = (get(spec, false, k), get(spec, true, k));
            if let (Some(n), Some(c)) = (normal.get_mut(k), centroid.get_mut(k)) {
                if k == axis {
                    *n = if side { 1.0 } else { -1.0 };
                    *c = if side { hi } else { lo };
                } else {
                    *c = 0.5 * (lo + hi);
                    area *= hi - lo;
                }
            }
        }
        Ok(FaceInfo {
            id: face,
            surface: SurfaceKind::Plane,
            normal: Some(normal),
            centroid,
            area,
        })
    }

    fn edge_endpoints(&self, edge: EdgeId) -> KernelResult<[[f64; 3]; 2]> {
        let (body, local) = Self::decode(edge.0);
        let spec = self.body(body)?;
        let &(along, s1, s2) =
            local
                .checked_sub(6)
                .and_then(|i| EDGES.get(i))
                .ok_or(KernelError::NotFound {
                    kind: "edge",
                    id: edge.0,
                })?;
        let (a1, a2) = ((along + 1) % 3, (along + 2) % 3);
        let mut p = [[0.0; 3]; 2];
        for (end, q) in p.iter_mut().enumerate() {
            for k in 0..3 {
                let v = if k == along {
                    get(spec, end == 1, k)
                } else if k == a1 {
                    get(spec, s1, k)
                } else {
                    get(spec, s2, k)
                };
                if let Some(slot) = q.get_mut(k) {
                    *slot = v;
                }
            }
            let _ = a2;
        }
        Ok(p)
    }

    fn mass_properties(&self, body: BodyId) -> KernelResult<MassProperties> {
        let s = self.body(body)?;
        let d: Vec<f64> = (0..3).map(|k| get(s, true, k) - get(s, false, k)).collect();
        let (x, y, z) = (
            d.first().copied().unwrap_or(0.0),
            d.get(1).copied().unwrap_or(0.0),
            d.get(2).copied().unwrap_or(0.0),
        );
        let c = (0..3)
            .map(|k| 0.5 * (get(s, true, k) + get(s, false, k)))
            .collect::<Vec<_>>();
        Ok(MassProperties {
            volume: x * y * z,
            area: 2.0 * (x * y + y * z + z * x),
            centroid: [
                c.first().copied().unwrap_or(0.0),
                c.get(1).copied().unwrap_or(0.0),
                c.get(2).copied().unwrap_or(0.0),
            ],
            exact: true,
        })
    }

    fn tessellate(&self, body: BodyId, _chord: f64) -> KernelResult<TriMesh> {
        let spec = self.body(body)?;
        let positions = Self::corners(spec).to_vec();
        // Corner index bits: x = 1, y = 2, z = 4. Two triangles per face, outward CCW.
        let quads: [(usize, [u32; 4]); 6] = [
            (0, [0, 4, 6, 2]),
            (1, [1, 3, 7, 5]),
            (2, [0, 1, 5, 4]),
            (3, [2, 6, 7, 3]),
            (4, [0, 2, 3, 1]),
            (5, [4, 5, 7, 6]),
        ];
        let mut triangles = Vec::with_capacity(12);
        let mut face_tags = Vec::with_capacity(12);
        for (local, [a, b, c, d]) in quads {
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
            let tag = FaceTag(Self::face_id(body, local).0);
            face_tags.extend([tag, tag]);
        }
        Ok(TriMesh {
            positions,
            triangles,
            face_tags,
        })
    }

    fn check(&self, body: BodyId) -> KernelResult<Validity> {
        let s = self.body(body)?;
        let ok = (0..3).all(|k| get(s, true, k) > get(s, false, k));
        Ok(Validity {
            ok,
            issues: if ok {
                Vec::new()
            } else {
                vec!["empty box".into()]
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_matches_exact_mass() {
        let mut k = BoxKernel::new();
        let out = k
            .make_box(&BoxSpec {
                min: [0.0; 3],
                max: [100.0, 60.0, 40.0],
            })
            .unwrap();
        let mesh = k.tessellate(out.body, 0.1).unwrap();
        let mass = k.mass_properties(out.body).unwrap();
        assert!(mesh.watertightness(1e-9).is_closed());
        assert!((mesh.volume() - mass.volume).abs() < 1e-9);
        assert!((mesh.area() - mass.area).abs() < 1e-9);
        assert_eq!(mass.volume, 240_000.0);
    }

    #[test]
    fn face_info_and_edges_are_consistent() {
        let mut k = BoxKernel::new();
        let b = k
            .make_box(&BoxSpec {
                min: [0.0; 3],
                max: [100.0, 60.0, 40.0],
            })
            .unwrap()
            .body;
        let top = k.faces(b).unwrap()[5];
        let info = k.face_info(top).unwrap();
        assert_eq!(info.normal, Some([0.0, 0.0, 1.0]));
        assert_eq!(info.centroid, [50.0, 30.0, 40.0]);
        assert_eq!(info.area, 6000.0);
        let lengths: f64 = k
            .edges(b)
            .unwrap()
            .into_iter()
            .map(|e| {
                let [p, q] = k.edge_endpoints(e).unwrap();
                ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt()
            })
            .sum();
        assert_eq!(lengths, 4.0 * (100.0 + 60.0 + 40.0));
    }
}
