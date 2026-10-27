//! Hostile parameters through `Guarded<ArrisKernel>`: errors are fine, panics are not.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use kubit_kernel_api::{BoxSpec, Guarded, Kernel, KernelError};
use kubit_kernel_arris::ArrisKernel;
use proptest::prelude::*;

fn hostile() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
        Just(0.0),
        Just(-0.0),
        Just(-1.0),
        Just(1e-12),
        Just(1e9),
        Just(f64::MAX),
        -1.0e3..1.0e3_f64,
        0.1..200.0_f64,
    ]
}

fn no_panic<T: std::fmt::Debug>(r: &Result<T, KernelError>) -> bool {
    !matches!(r, Err(KernelError::Panicked { .. } | KernelError::Poisoned))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn make_box_never_panics(min in prop::array::uniform3(hostile()), max in prop::array::uniform3(hostile())) {
        let mut k = Guarded::new(ArrisKernel::new());
        let r = k.make_box(&BoxSpec { min, max });
        prop_assert!(no_panic(&r), "{r:?}");
        prop_assert!(!k.is_poisoned());
    }

    #[test]
    fn offset_faces_never_panics(size in prop::array::uniform3(hostile()), d in hostile(), pick in 0usize..6) {
        let mut k = Guarded::new(ArrisKernel::new());
        let max = size;
        if let Ok(out) = k.make_box(&BoxSpec { min: [0.0; 3], max }) {
            let faces = k.faces(out.body).unwrap();
            let face = faces[pick % faces.len()];
            let r = k.offset_faces(out.body, &[face], d);
            prop_assert!(no_panic(&r), "{r:?}");
            if let Ok(moved) = r {
                prop_assert!(k.check(moved.body).unwrap().ok);
            }
        }
        prop_assert!(!k.is_poisoned());
    }
}
