//! Pick ids: every face of every uploaded mesh gets a global `u32`.
//!
//! The id pass writes that `u32` into an `R32Uint` target (0 = background), and the
//! same id drives the hover/selection highlight in the shaded pass. A mesh owns the
//! contiguous range `base .. base + face_count`; `FaceTag(u64)` values map to local
//! indices in order of first appearance.

use kubit_mesh::{FaceTag, TriMesh};
use std::collections::BTreeMap;

/// Caller-chosen identifier of an uploaded mesh (a body, typically).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshId(pub u64);

/// The id written for "no face".
pub const BACKGROUND_ID: u32 = 0;

/// Faces of one mesh: their tags, indexed by local id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FaceTable {
    tags: Vec<FaceTag>,
    local: BTreeMap<FaceTag, u32>,
}

impl FaceTable {
    /// Distinct tags of `mesh`, in order of first appearance, and the local index of
    /// every triangle. `None` if the mesh has more than `u32::MAX` faces.
    pub fn build(mesh: &TriMesh) -> Option<(Self, Vec<u32>)> {
        let mut table = Self::default();
        let mut per_triangle = Vec::with_capacity(mesh.face_tags.len());
        for &tag in &mesh.face_tags {
            let next = u32::try_from(table.tags.len()).ok()?;
            let local = *table.local.entry(tag).or_insert(next);
            if local == next {
                table.tags.push(tag);
            }
            per_triangle.push(local);
        }
        Some((table, per_triangle))
    }

    /// Number of distinct faces.
    pub fn len(&self) -> u32 {
        u32::try_from(self.tags.len()).unwrap_or(u32::MAX)
    }

    /// No faces.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// Local index of a tag.
    pub fn local(&self, tag: FaceTag) -> Option<u32> {
        self.local.get(&tag).copied()
    }

    /// Tag at a local index.
    pub fn tag(&self, local: u32) -> Option<FaceTag> {
        self.tags.get(local as usize).copied()
    }
}

/// Allocates global id ranges to meshes and maps ids back.
#[derive(Clone, Debug, Default)]
pub struct IdMap {
    /// Next free id; ids start at 1 because 0 is the background.
    next: u32,
    /// Range start → (mesh, range length).
    ranges: BTreeMap<u32, (MeshId, u32)>,
}

impl IdMap {
    /// Reserves `count` ids for `mesh`, returning the base, or `None` when the id space
    /// is exhausted (call [`IdMap::compact`] with the live meshes and retry).
    pub fn allocate(&mut self, mesh: MeshId, count: u32) -> Option<u32> {
        let base = self.next.max(1);
        let end = base.checked_add(count)?;
        self.next = end;
        self.ranges.insert(base, (mesh, count));
        Some(base)
    }

    /// Forgets the range starting at `base`.
    pub fn release(&mut self, base: u32) {
        self.ranges.remove(&base);
    }

    /// Clears every range (the caller then re-allocates the live meshes).
    pub fn compact(&mut self) {
        self.ranges.clear();
        self.next = 1;
    }

    /// Mesh and local face index of a global id.
    pub fn decode(&self, id: u32) -> Option<(MeshId, u32)> {
        if id == BACKGROUND_ID {
            return None;
        }
        let (&base, &(mesh, count)) = self.ranges.range(..=id).next_back()?;
        let local = id - base;
        (local < count).then_some((mesh, local))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_packing_round_trips() {
        let mut map = IdMap::default();
        let a = map.allocate(MeshId(10), 6).unwrap();
        let b = map.allocate(MeshId(20), 3).unwrap();
        assert_eq!(a, 1);
        for local in 0..6 {
            assert_eq!(map.decode(a + local), Some((MeshId(10), local)));
        }
        for local in 0..3 {
            assert_eq!(map.decode(b + local), Some((MeshId(20), local)));
        }
        assert_eq!(map.decode(BACKGROUND_ID), None);
        assert_eq!(map.decode(b + 3), None);
        map.release(a);
        assert_eq!(map.decode(a), None);
        assert_eq!(map.decode(b), Some((MeshId(20), 0)));
    }

    #[test]
    fn allocation_fails_cleanly_when_exhausted() {
        let mut map = IdMap::default();
        assert!(map.allocate(MeshId(1), u32::MAX - 1).is_some());
        assert!(map.allocate(MeshId(2), 5).is_none());
        map.compact();
        assert_eq!(map.allocate(MeshId(2), 5), Some(1));
    }

    #[test]
    fn face_table_maps_large_tags_to_dense_indices() {
        let mesh = TriMesh {
            positions: vec![],
            triangles: vec![[0, 0, 0]; 4],
            face_tags: vec![
                FaceTag(u64::MAX),
                FaceTag(u64::MAX),
                FaceTag(1 << 40),
                FaceTag(u64::MAX),
            ],
            edges: Vec::new(),
        };
        let (table, per_tri) = FaceTable::build(&mesh).unwrap();
        assert_eq!(per_tri, vec![0, 0, 1, 0]);
        assert_eq!(table.len(), 2);
        assert_eq!(table.tag(1), Some(FaceTag(1 << 40)));
        assert_eq!(table.local(FaceTag(u64::MAX)), Some(0));
    }
}
