//! Deterministic hostile parameter sets for case 15, generated with proptest.

use crate::Hostile;
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;

pub const CASES_PER_FAMILY: usize = 256;

/// A float that is often degenerate: NaN, ±∞, 0, negative, tiny, huge, or ordinary.
fn hostile_f64() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
        Just(0.0),
        Just(-0.0),
        Just(-1.0),
        Just(1e-12),
        Just(f64::MIN_POSITIVE),
        Just(1e9),
        Just(f64::MAX),
        -1.0e3..1.0e3_f64,
        0.1..200.0_f64,
    ]
}

pub fn arity(family: Hostile) -> usize {
    match family {
        Hostile::BoxMinusCylinder | Hostile::FilletEdge => 4,
        Hostile::PushPull => 5,
    }
}

/// The parameter vectors, identical on every run and every backend.
pub fn parameter_sets(family: Hostile) -> Vec<Vec<f64>> {
    let mut runner = TestRunner::deterministic();
    let strategy = proptest::collection::vec(hostile_f64(), arity(family));
    let mut out = Vec::with_capacity(CASES_PER_FAMILY);
    while out.len() < CASES_PER_FAMILY {
        match strategy.new_tree(&mut runner) {
            Ok(tree) => out.push(tree.current()),
            Err(_) => break,
        }
    }
    out
}

pub fn family_for_test(test: u8) -> Option<Hostile> {
    match test {
        1 => Some(Hostile::BoxMinusCylinder),
        5 => Some(Hostile::FilletEdge),
        9 => Some(Hostile::PushPull),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_are_deterministic_and_hostile() {
        let a = parameter_sets(Hostile::PushPull);
        let b = parameter_sets(Hostile::PushPull);
        assert_eq!(a.len(), CASES_PER_FAMILY);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert!(a.iter().flatten().any(|v| v.is_nan()));
        assert!(a.iter().flatten().any(|v| v.is_infinite()));
    }
}
