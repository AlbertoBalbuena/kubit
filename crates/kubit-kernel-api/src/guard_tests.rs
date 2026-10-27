//! Guard behaviour: input validation, panic capture, poisoning, result checks,
//! history reconstruction, and zero panics under hostile input.

use crate::testing::BoxKernel;
use crate::*;
use proptest::prelude::*;

fn bx(max: [f64; 3]) -> BoxSpec {
    BoxSpec { min: [0.0; 3], max }
}

#[test]
fn box_then_push_pull_oracle() {
    let mut k = Guarded::new(BoxKernel::new());
    let b = k.make_box(&bx([100.0, 60.0, 40.0])).unwrap();
    let top = k.faces(b.body).unwrap()[5];
    let pushed = k.offset_faces(b.body, &[top], 10.0).unwrap();
    let mass = k.mass_properties(pushed.body).unwrap();
    assert_eq!(mass.volume, 100.0 * 60.0 * 50.0);
    assert_eq!(pushed.history.source, HistorySource::Native);
    assert_eq!(pushed.history.counts(), (0, 19, 0));
    let pulled = k
        .offset_faces(pushed.body, &[k.faces(pushed.body).unwrap()[5]], -20.0)
        .unwrap();
    assert_eq!(
        k.mass_properties(pulled.body).unwrap().volume,
        100.0 * 60.0 * 30.0
    );
}

#[test]
fn hostile_numbers_are_rejected_before_the_backend() {
    let mut k = Guarded::new(BoxKernel::new());
    for bad in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e-9, 2e6] {
        let r = k.make_box(&bx([bad, 10.0, 10.0]));
        assert!(
            matches!(r, Err(KernelError::InvalidInput { .. })),
            "{bad}: {r:?}"
        );
    }
    let b = k.make_box(&bx([10.0; 3])).unwrap().body;
    let f = k.faces(b).unwrap()[0];
    assert!(matches!(
        k.offset_faces(b, &[f], f64::NAN),
        Err(KernelError::InvalidInput { .. })
    ));
    assert!(matches!(
        k.offset_faces(b, &[f], 0.0),
        Err(KernelError::InvalidInput { .. })
    ));
    assert!(matches!(
        k.offset_faces(b, &[], 1.0),
        Err(KernelError::InvalidInput { .. })
    ));
    assert!(matches!(
        k.tessellate(b, 0.0),
        Err(KernelError::InvalidInput { .. })
    ));
}

#[test]
fn collapse_is_a_clean_error() {
    let mut k = Guarded::new(BoxKernel::new());
    let b = k.make_box(&bx([10.0; 3])).unwrap().body;
    let top = k.faces(b).unwrap()[5];
    assert!(matches!(
        k.offset_faces(b, &[top], -10.0),
        Err(KernelError::Failed { .. })
    ));
}

/// A backend that panics in `make_box` and returns garbage elsewhere.
struct Faulty(BoxKernel, bool);

impl Kernel for Faulty {
    fn info(&self) -> KernelInfo {
        KernelInfo {
            name: "faulty".into(),
            version: "0".into(),
            capabilities: Capabilities::default(),
        }
    }
    fn make_box(&mut self, spec: &BoxSpec) -> KernelResult<OpOutput> {
        if self.1 {
            #[allow(clippy::panic)]
            {
                panic!("boom");
            }
        }
        self.0.make_box(spec)
    }
    fn offset_faces(&mut self, body: BodyId, faces: &[FaceId], d: f64) -> KernelResult<OpOutput> {
        self.0.offset_faces(body, faces, d)
    }
    fn delete_body(&mut self, body: BodyId) -> KernelResult<()> {
        self.0.delete_body(body)
    }
    fn faces(&self, body: BodyId) -> KernelResult<Vec<FaceId>> {
        self.0.faces(body)
    }
    fn edges(&self, body: BodyId) -> KernelResult<Vec<EdgeId>> {
        self.0.edges(body)
    }
    fn face_info(&self, face: FaceId) -> KernelResult<FaceInfo> {
        self.0.face_info(face)
    }
    fn edge_endpoints(&self, edge: EdgeId) -> KernelResult<[[f64; 3]; 2]> {
        self.0.edge_endpoints(edge)
    }
    fn mass_properties(&self, body: BodyId) -> KernelResult<MassProperties> {
        self.0.mass_properties(body)
    }
    fn tessellate(&self, body: BodyId, chord: f64) -> KernelResult<TriMesh> {
        let mut m = self.0.tessellate(body, chord)?;
        m.triangles.push([0, 1, 999]);
        m.face_tags.push(kubit_mesh::FaceTag(0));
        Ok(m)
    }
    fn check(&self, body: BodyId) -> KernelResult<Validity> {
        self.0.check(body)
    }
}

#[test]
fn panic_becomes_error_and_poisons() {
    let mut k = Guarded::new(Faulty(BoxKernel::new(), true));
    let r = k.make_box(&bx([1.0; 3]));
    assert!(
        matches!(r, Err(KernelError::Panicked { op: "make_box", .. })),
        "{r:?}"
    );
    assert!(k.is_poisoned());
    assert_eq!(k.faces(BodyId(1)), Err(KernelError::Poisoned));
    k.reset(Faulty(BoxKernel::new(), false));
    assert!(k.make_box(&bx([1.0; 3])).is_ok());
}

#[test]
fn invalid_mesh_is_rejected() {
    let mut k = Guarded::new(Faulty(BoxKernel::new(), false));
    let b = k.make_box(&bx([1.0; 3])).unwrap().body;
    assert!(matches!(
        k.tessellate(b, 0.1),
        Err(KernelError::InvalidResult { .. })
    ));
}

#[test]
fn history_is_reconstructed_without_native_support() {
    // `Faulty` declares no native history, so the guard rebuilds it.
    let mut k = Guarded::new(Faulty(BoxKernel::new(), false));
    let b = k.make_box(&bx([100.0, 60.0, 40.0])).unwrap();
    assert_eq!(b.history.source, HistorySource::Reconstructed);
    assert_eq!(b.history.counts(), (6, 0, 0));
    let top = k.faces(b.body).unwrap()[5];
    let p = k.offset_faces(b.body, &[top], 10.0).unwrap();
    assert_eq!(p.history.source, HistorySource::Reconstructed);
    // The top moved (declared); the bottom and the four sides keep their planes.
    assert_eq!(p.history.counts(), (0, 6, 0));
    let new_top = k.faces(p.body).unwrap()[5];
    assert_eq!(
        p.history.images_of(EntityRef::Face(top)),
        &[EntityRef::Face(new_top)]
    );
}

#[test]
fn unsupported_is_reported() {
    let mut k = Guarded::new(BoxKernel::new());
    let a = k.make_box(&bx([1.0; 3])).unwrap().body;
    let b = k.make_box(&bx([2.0; 3])).unwrap().body;
    assert!(matches!(
        k.boolean(BooleanOp::Union, a, b),
        Err(KernelError::Unsupported { .. })
    ));
}

fn hostile() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
        Just(0.0),
        Just(-0.0),
        Just(f64::MIN_POSITIVE),
        Just(1e300),
        -1e7..1e7_f64,
    ]
}

proptest! {
    #[test]
    fn box_and_push_pull_never_panic(
        min in proptest::array::uniform3(hostile()),
        max in proptest::array::uniform3(hostile()),
        face in 0usize..8,
        d in hostile(),
    ) {
        let mut k = Guarded::new(BoxKernel::new());
        if let Ok(b) = k.make_box(&BoxSpec { min, max }) {
            let faces = k.faces(b.body).unwrap_or_default();
            let f = faces.get(face).copied().unwrap_or(FaceId(u64::MAX));
            let _ = k.offset_faces(b.body, &[f], d);
        }
        prop_assert!(!k.is_poisoned());
    }
}
