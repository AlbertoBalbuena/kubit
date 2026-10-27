//! Sanity check: the hostile families succeed on ordinary parameters, so a
//! clean error on hostile input is the kernel's verdict and not the adapter's.
use backend_arris::Arris;
use harness::{Backend, Hostile};

#[test]
fn ordinary_parameters_pass() {
    assert!(Arris.hostile(Hostile::BoxMinusCylinder, &[100.0, 60.0, 40.0, 10.0]).is_ok());
    assert!(Arris.hostile(Hostile::FilletEdge, &[100.0, 60.0, 40.0, 5.0]).is_ok());
    assert!(Arris.hostile(Hostile::PushPull, &[100.0, 60.0, 40.0, 5.0, 10.0]).is_ok());
    assert!(Arris.hostile(Hostile::PushPull, &[100.0, 60.0, 40.0, 5.0, -10.0]).is_ok());
}

#[test]
fn degenerate_parameters_are_clean_errors() {
    assert!(Arris.hostile(Hostile::BoxMinusCylinder, &[f64::NAN, 60.0, 40.0, 10.0]).is_err());
    assert!(Arris.hostile(Hostile::FilletEdge, &[100.0, 60.0, 40.0, 0.0]).is_err());
}
