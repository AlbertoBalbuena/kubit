//! Triangle meshes produced by tessellating B-rep bodies (L1).
//!
//! Positions are in millimetres, triangles are wound counter-clockwise seen from
//! outside, and every triangle carries the tag of the B-rep face it came from so
//! the viewport can pick faces. Layer and rules: `docs/ARCHITECTURE.md`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Opaque tag of the B-rep face a triangle belongs to (a kernel face id).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FaceTag(pub u64);

/// Axis-aligned bounding box in millimetres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    /// Minimum corner.
    pub min: [f64; 3],
    /// Maximum corner.
    pub max: [f64; 3],
}

impl Aabb {
    /// Length of the diagonal; 0 for a point.
    pub fn diagonal(&self) -> f64 {
        let d = [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    /// Centre point.
    pub fn center(&self) -> [f64; 3] {
        [
            0.5 * (self.min[0] + self.max[0]),
            0.5 * (self.min[1] + self.max[1]),
            0.5 * (self.min[2] + self.max[2]),
        ]
    }
}

/// A triangle mesh with one face tag per triangle.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TriMesh {
    /// Vertex positions, mm.
    pub positions: Vec<[f64; 3]>,
    /// Vertex indices, CCW seen from outside.
    pub triangles: Vec<[u32; 3]>,
    /// `face_tags[i]` is the B-rep face of `triangles[i]`; same length as `triangles`.
    pub face_tags: Vec<FaceTag>,
}

/// Why a mesh is structurally unusable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshDefect {
    /// A triangle refers to a vertex that does not exist.
    IndexOutOfRange {
        /// Triangle index.
        triangle: usize,
    },
    /// A position is NaN or infinite.
    NonFinitePosition {
        /// Vertex index.
        vertex: usize,
    },
    /// `face_tags` and `triangles` differ in length.
    TagCountMismatch,
}

/// Closedness of a mesh after welding coincident vertices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watertightness {
    /// Edges used by exactly one triangle.
    pub boundary_edges: usize,
    /// Edges used by more than two triangles.
    pub nonmanifold_edges: usize,
    /// Directed edges used twice in the same direction (flipped neighbours).
    pub misoriented_edges: usize,
}

impl Watertightness {
    /// Closed, manifold and consistently oriented.
    pub fn is_closed(&self) -> bool {
        self.boundary_edges == 0 && self.nonmanifold_edges == 0 && self.misoriented_edges == 0
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl TriMesh {
    /// Checks indices, finiteness and tag count.
    pub fn validate(&self) -> Result<(), MeshDefect> {
        if self.face_tags.len() != self.triangles.len() {
            return Err(MeshDefect::TagCountMismatch);
        }
        if let Some(vertex) = self
            .positions
            .iter()
            .position(|p| p.iter().any(|c| !c.is_finite()))
        {
            return Err(MeshDefect::NonFinitePosition { vertex });
        }
        let n = self.positions.len();
        if let Some(triangle) = self
            .triangles
            .iter()
            .position(|t| t.iter().any(|&i| i as usize >= n))
        {
            return Err(MeshDefect::IndexOutOfRange { triangle });
        }
        Ok(())
    }

    /// The three corners of a triangle, or `None` if an index is out of range.
    pub fn corners(&self, triangle: usize) -> Option<[[f64; 3]; 3]> {
        let t = self.triangles.get(triangle)?;
        Some([
            *self.positions.get(t[0] as usize)?,
            *self.positions.get(t[1] as usize)?,
            *self.positions.get(t[2] as usize)?,
        ])
    }

    /// Enclosed volume by the divergence theorem (meaningful for closed meshes).
    pub fn volume(&self) -> f64 {
        (0..self.triangles.len())
            .filter_map(|i| self.corners(i))
            .map(|[a, b, c]| dot(a, cross(b, c)) / 6.0)
            .sum()
    }

    /// Total surface area.
    pub fn area(&self) -> f64 {
        (0..self.triangles.len())
            .filter_map(|i| self.corners(i))
            .map(|[a, b, c]| {
                let n = cross(sub(b, a), sub(c, a));
                0.5 * dot(n, n).sqrt()
            })
            .sum()
    }

    /// Bounding box, or `None` for an empty mesh.
    pub fn aabb(&self) -> Option<Aabb> {
        let first = *self.positions.first()?;
        let mut b = Aabb {
            min: first,
            max: first,
        };
        for p in &self.positions {
            for ((lo, hi), c) in b.min.iter_mut().zip(b.max.iter_mut()).zip(p) {
                *lo = lo.min(*c);
                *hi = hi.max(*c);
            }
        }
        Some(b)
    }

    /// Appends `other`, renumbering its indices.
    pub fn append(&mut self, other: &TriMesh) {
        let base = u32::try_from(self.positions.len()).unwrap_or(u32::MAX);
        self.positions.extend_from_slice(&other.positions);
        self.triangles.extend(
            other
                .triangles
                .iter()
                .map(|t| t.map(|i| i.saturating_add(base))),
        );
        self.face_tags.extend_from_slice(&other.face_tags);
    }

    /// Edge-use statistics after welding vertices closer than `weld` mm.
    pub fn watertightness(&self, weld: f64) -> Watertightness {
        let weld = if weld.is_finite() && weld > 0.0 {
            weld
        } else {
            1e-6
        };
        let mut cells: BTreeMap<[i64; 3], u32> = BTreeMap::new();
        let ids: Vec<u32> = self
            .positions
            .iter()
            .map(|p| {
                // Snapping to a grid cell is the point of the cast; out-of-range
                // values saturate, which only merges far-away garbage vertices.
                #[allow(clippy::cast_possible_truncation)]
                let key = p.map(|c| (c / weld).round() as i64);
                let next = u32::try_from(cells.len()).unwrap_or(u32::MAX);
                *cells.entry(key).or_insert(next)
            })
            .collect();
        let mut undirected: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        let mut directed: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        for t in &self.triangles {
            let w = t.map(|i| ids.get(i as usize).copied().unwrap_or(u32::MAX));
            let [w0, w1, w2] = w;
            for (a, b) in [(w0, w1), (w1, w2), (w2, w0)] {
                if a == b {
                    continue;
                }
                *directed.entry((a, b)).or_insert(0) += 1;
                *undirected.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        Watertightness {
            boundary_edges: undirected.values().filter(|&&n| n == 1).count(),
            nonmanifold_edges: undirected.values().filter(|&&n| n > 2).count(),
            misoriented_edges: directed.values().filter(|&&n| n > 1).count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Unit cube, 12 triangles, outward CCW.
    pub(crate) fn cube(size: f64) -> TriMesh {
        let s = size;
        let positions = vec![
            [0.0, 0.0, 0.0],
            [s, 0.0, 0.0],
            [s, s, 0.0],
            [0.0, s, 0.0],
            [0.0, 0.0, s],
            [s, 0.0, s],
            [s, s, s],
            [0.0, s, s],
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let face_tags = (0..12).map(|i| FaceTag(i / 2)).collect();
        TriMesh {
            positions,
            triangles,
            face_tags,
        }
    }

    #[test]
    fn cube_oracle() {
        let m = cube(10.0);
        assert!(m.validate().is_ok());
        assert!((m.volume() - 1000.0).abs() < 1e-9);
        assert!((m.area() - 600.0).abs() < 1e-9);
        assert!(m.watertightness(1e-6).is_closed());
        let b = m.aabb().unwrap();
        assert_eq!(b.center(), [5.0, 5.0, 5.0]);
    }

    #[test]
    fn defects_are_reported() {
        let mut m = cube(1.0);
        m.triangles.pop();
        assert_eq!(m.validate(), Err(MeshDefect::TagCountMismatch));
        m.face_tags.pop();
        assert_eq!(m.watertightness(1e-6).boundary_edges, 3);
        m.triangles[0] = [0, 1, 99];
        assert_eq!(
            m.validate(),
            Err(MeshDefect::IndexOutOfRange { triangle: 0 })
        );
    }

    #[test]
    fn append_renumbers() {
        let mut a = cube(1.0);
        a.append(&cube(1.0));
        assert_eq!(a.positions.len(), 16);
        assert!(a.validate().is_ok());
        assert!((a.volume() - 2.0).abs() < 1e-12);
    }

    proptest! {
        #[test]
        fn hostile_meshes_never_panic(
            positions in proptest::collection::vec(proptest::array::uniform3(proptest::num::f64::ANY), 0..8),
            triangles in proptest::collection::vec(proptest::array::uniform3(0u32..12), 0..8),
            weld in proptest::num::f64::ANY,
        ) {
            let tags = vec![FaceTag(0); triangles.len()];
            let m = TriMesh { positions, triangles, face_tags: tags };
            let _ = m.validate();
            let _ = (m.volume(), m.area(), m.aabb(), m.watertightness(weld));
        }
    }
}
