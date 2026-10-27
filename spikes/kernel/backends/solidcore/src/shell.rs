//! Shell (hollow) through `api_shell_brep_full`, which needs caller-owned notification
//! stores and identity closures. The identity scheme follows the one in solidcore's
//! difftest (`crates/difftest/src/units/offset_offset_executive_p2.rs`, ~1416-1660):
//! disjoint namespaces in the top bits of every `ObjectKey`.

use std::collections::BTreeMap;

use harness::{SpikeError, SpikeResult};
use solidcore::curve::curve::Curve;
use solidcore::error::EntityRef;
use solidcore::geom::attribute::{AttributeStore, ObjectKey};
use solidcore::geom::object::{FaceKey, ObjectRef};
use solidcore::offset::offset_executive::p2::{
    api_shell_brep_full, OffsetNotifications, OffsetOptions, ShellCopyIdentities, ShellOptions,
};
use solidcore::query::api_queries::BrepRef;
use solidcore::sweep::topology_sweep::store::SweepIdentities;
use solidcore::topology::topology::Topology;
use solidcore::topology::{Brep, BrepId, EdgeId, FaceId, Region, RegionId, Shell};
use solidcore::Kernel;

/// Adapter handle of the source B-rep in face selections.
const SOURCE: BrepId = BrepId::from_index(17);
const NS_SOURCE: u64 = 1;
const NS_RESULT: u64 = 3;

fn notification_key(entity: EntityRef, namespace: u64) -> ObjectRef {
    let (index, type_code) = match entity {
        EntityRef::Brep(id) => (id.index(), solidcore::geom::a_object::BREP_TYPE),
        EntityRef::Region(id) => (id.index(), Region::type_code()),
        EntityRef::Shell(id) => (id.index(), Shell::type_code()),
        EntityRef::Face(id) => (id.index(), solidcore::topology::face::p2::TYPE),
        EntityRef::Faceuse(id) => (id.index(), 16022),
        EntityRef::Loopuse(id) => (id.index(), solidcore::topology::loopuse::TYPE),
        EntityRef::Edge(id) => (id.index(), solidcore::topology::edge::TYPE),
        EntityRef::Edgeuse(id) => (id.index(), solidcore::topology::edgeuse::p1::TYPE),
        EntityRef::Vertex(id) => (id.index(), solidcore::topology::vertex::TYPE),
        EntityRef::Vertexuse(id) => (id.index(), solidcore::topology::vertexuse::TYPE),
        EntityRef::Loop(id) => (id.index(), solidcore::topology::topology::TYPE),
    };
    ObjectRef { key: ObjectKey((namespace << 60) | ((type_code as u64) << 32) | u64::from(index)), type_code }
}

fn original_key(entity: EntityRef) -> ObjectRef {
    notification_key(entity, NS_SOURCE)
}

fn result_key(entity: EntityRef) -> ObjectRef {
    notification_key(entity, NS_RESULT)
}

fn curve_key(namespace: u64, edge: EdgeId, type_code: i64) -> ObjectRef {
    ObjectRef { key: ObjectKey((namespace << 60) | ((type_code as u64) << 32) | u64::from(edge.index())), type_code }
}

fn original_curve_key(edge: EdgeId, type_code: i64) -> ObjectRef {
    curve_key(5, edge, type_code)
}

fn result_curve_key(edge: EdgeId, type_code: i64) -> ObjectRef {
    curve_key(4, edge, type_code)
}

fn boolean_source_lists(result: bool, region: RegionId) -> [ObjectRef; 2] {
    let namespace = if result { 7_u64 } else { 6_u64 };
    [0_u64, 1].map(|list| ObjectRef {
        key: ObjectKey((namespace << 60) | (list << 32) | u64::from(region.index())),
        type_code: 0,
    })
}

/// Identities of merge-created B-reps and of the shell's working copy (namespace 0).
struct MergeBodies;

impl MergeBodies {
    fn key(class: u64, brep: BrepId, kind: u64, index: u32) -> ObjectKey {
        ObjectKey((class << 58) | ((u64::from(brep.index()) & 0xfff) << 46) | ((kind & 0x3fff) << 32) | u64::from(index))
    }
}

impl SweepIdentities for MergeBodies {
    fn topology(&self, brep: BrepId, entity: EntityRef) -> ObjectRef {
        let local = notification_key(entity, 0);
        ObjectRef {
            key: Self::key(1, brep, local.type_code as u64, (local.key.0 & 0xffff_ffff) as u32),
            type_code: local.type_code,
        }
    }
    fn owned_auxiliary_objects(&self, brep: BrepId, model: &Brep) -> Vec<ObjectKey> {
        model
            .edges()
            .iter()
            .filter_map(|(edge, e)| Some(self.curve(brep, edge, e.curve.as_ref()?).key))
            .collect()
    }
    fn curve(&self, brep: BrepId, edge: EdgeId, curve: &Curve) -> ObjectRef {
        let type_code = curve.type_code();
        ObjectRef { key: Self::key(2, brep, type_code as u64, edge.index()), type_code }
    }
    fn new_curve(&self, brep: BrepId, edge: EdgeId, type_code: i64) -> solidcore::Result<ObjectRef> {
        Ok(ObjectRef { key: Self::key(2, brep, type_code as u64, edge.index()), type_code })
    }
    fn source_lists(&self, brep: BrepId, region: RegionId) -> solidcore::Result<[ObjectRef; 2]> {
        Ok([0_u64, 1].map(|list| ObjectRef { key: Self::key(3, brep, list, region.index()), type_code: 0 }))
    }
}

fn copy_key(entity: EntityRef) -> ObjectRef {
    let local = notification_key(entity, 0);
    ObjectRef {
        key: MergeBodies::key(0, BrepId::from_index(0), local.type_code as u64, (local.key.0 & 0xffff_ffff) as u32),
        type_code: local.type_code,
    }
}

fn copy_curve_key(edge: EdgeId, type_code: i64) -> ObjectRef {
    ObjectRef { key: MergeBodies::key(0, BrepId::from_index(1), type_code as u64, edge.index()), type_code }
}

fn copy_source_lists(region: RegionId) -> [ObjectRef; 2] {
    [0_u64, 1].map(|list| ObjectRef { key: MergeBodies::key(0, BrepId::from_index(2), list, region.index()), type_code: 0 })
}

/// Registers every topology record of `brep` in the notification store.
fn register(records: &mut BTreeMap<ObjectKey, Topology>, brep: &mut Brep, namespace: u64) {
    let mut add = |entity| {
        records.insert(notification_key(entity, namespace).key, Topology::new());
    };
    let root = EntityRef::Brep(BrepId::from_index(0));
    brep.set_key(notification_key(root, namespace).key);
    add(root);
    for (id, _) in brep.regions().iter() {
        add(EntityRef::Region(id));
    }
    for (id, _) in brep.shells().iter() {
        add(EntityRef::Shell(id));
    }
    for (id, _) in brep.faces().iter() {
        add(EntityRef::Face(id));
    }
    for (id, _) in brep.faceuses().iter() {
        add(EntityRef::Faceuse(id));
    }
    for (id, _) in brep.loopuses().iter() {
        add(EntityRef::Loopuse(id));
    }
    for (id, _) in brep.edges().iter() {
        add(EntityRef::Edge(id));
    }
    for (id, _) in brep.edgeuses().iter() {
        add(EntityRef::Edgeuse(id));
    }
    for (id, _) in brep.vertices().iter() {
        add(EntityRef::Vertex(id));
    }
    for (id, _) in brep.vertexuses().iter() {
        add(EntityRef::Vertexuse(id));
    }
}

/// Shells `source` by `distance` (negative = inward), removing `open` faces.
/// `merge_results` is needed: without it (and without `extend_convex_edges`) the kernel
/// returns `Ok` with the untrimmed offset faces, a zero-volume non-manifold sheet.
pub fn shell(k: &mut Kernel, mut source: Brep, open: &[FaceId], distance: f64) -> SpikeResult<Brep> {
    let offset = OffsetOptions { distance, extend_convex_edges: false, merge_results: true };
    let mut records = BTreeMap::new();
    let mut attributes = AttributeStore::default();
    register(&mut records, &mut source, NS_SOURCE);
    let result_brep = result_key(EntityRef::Brep(BrepId::from_index(0)));
    records.insert(result_brep.key, Topology::new());
    let mut notifications = OffsetNotifications {
        records: &mut records,
        attributes: &mut attributes,
        original_key_of: &original_key,
        result_key_of: &result_key,
        original_curve_ref: &original_curve_key,
        result_curve_ref: &result_curve_key,
        boolean_source_lists: &boolean_source_lists,
        result_brep,
        merge_identities: Some(&MergeBodies),
    };
    let faces: Vec<Option<FaceKey>> = open.iter().map(|f| Some((SOURCE, *f))).collect();
    let options = ShellOptions {
        offset,
        create_offset_solid: false,
        merge_sheet_side_walls: false,
    };
    api_shell_brep_full(
        k,
        Some(BrepRef { brep: &source, handle: SOURCE }),
        options,
        &faces,
        &ShellCopyIdentities { key_of: &copy_key, curve_ref: &copy_curve_key, boolean_source_lists: &copy_source_lists },
        &mut notifications,
    )
    .map_err(|e| {
        let tag = if e.is_unported() { "Unported: " } else { "" };
        SpikeError::Failed(format!("api_shell_brep_full: {tag}{e}"))
    })
}
