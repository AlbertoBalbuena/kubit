//! Modelling tolerances and approximate comparisons.

use crate::units::{Angle, Length};
use core::f64::consts::TAU;
use thiserror::Error;

/// Default linear tolerance: 1e-6 mm (one nanometre), far below any manufacturable feature
/// and well above `f64` noise for coordinates up to 1e6 mm.
pub const DEFAULT_LINEAR_MM: f64 = 1.0e-6;
/// Default angular tolerance: 1e-9 rad.
pub const DEFAULT_ANGULAR_RAD: f64 = 1.0e-9;

/// Invalid tolerance values.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum ToleranceError {
    /// Tolerances must be finite and strictly positive.
    #[error("tolerance must be finite and > 0, got {0}")]
    Invalid(f64),
}

/// Absolute comparison tolerances: `linear` in millimetres, `angular` in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Linear tolerance in millimetres.
    pub linear: f64,
    /// Angular tolerance in radians.
    pub angular: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            linear: DEFAULT_LINEAR_MM,
            angular: DEFAULT_ANGULAR_RAD,
        }
    }
}

impl Tolerance {
    /// Creates a tolerance, rejecting non-finite or non-positive values.
    pub fn new(linear: f64, angular: f64) -> Result<Self, ToleranceError> {
        for v in [linear, angular] {
            if !v.is_finite() || v <= 0.0 {
                return Err(ToleranceError::Invalid(v));
            }
        }
        Ok(Self { linear, angular })
    }

    /// `true` if two raw millimetre values differ by at most the linear tolerance.
    pub fn approx_eq(&self, a: f64, b: f64) -> bool {
        approx_eq(a, b, self.linear)
    }

    /// `true` if two lengths are equal within the linear tolerance.
    pub fn lengths_eq(&self, a: Length, b: Length) -> bool {
        approx_eq(a.mm(), b.mm(), self.linear)
    }

    /// `true` if a length is zero within the linear tolerance.
    pub fn is_zero_length(&self, a: Length) -> bool {
        a.mm().abs() <= self.linear
    }

    /// `true` if two angles are equal within the angular tolerance (no wrapping).
    pub fn angles_eq(&self, a: Angle, b: Angle) -> bool {
        approx_eq(a.rad(), b.rad(), self.angular)
    }

    /// Like [`Tolerance::angles_eq`] but treating angles that differ by whole turns as equal.
    pub fn angles_eq_modulo_turn(&self, a: Angle, b: Angle) -> bool {
        let d = (a.rad() - b.rad()).rem_euclid(TAU);
        d <= self.angular || TAU - d <= self.angular
    }
}

/// Absolute comparison: `|a - b| <= tol`. Non-finite inputs are never equal.
pub fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= tol
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_documented_values() {
        let t = Tolerance::default();
        assert_eq!(t.linear, 1.0e-6);
        assert_eq!(t.angular, 1.0e-9);
    }

    #[test]
    fn new_rejects_bad_values() {
        assert!(Tolerance::new(0.0, 1e-9).is_err());
        assert!(Tolerance::new(1e-6, -1.0).is_err());
        assert!(Tolerance::new(f64::NAN, 1e-9).is_err());
        assert!(Tolerance::new(f64::INFINITY, 1e-9).is_err());
        assert!(Tolerance::new(1e-3, 1e-3).is_ok());
    }

    #[test]
    fn approx_eq_works_and_rejects_non_finite() {
        assert!(approx_eq(1.0, 1.0 + 1e-7, 1e-6));
        assert!(!approx_eq(1.0, 1.1, 1e-6));
        assert!(!approx_eq(f64::NAN, f64::NAN, 1.0));
        assert!(!approx_eq(f64::INFINITY, f64::INFINITY, 1.0));
    }

    #[test]
    fn length_and_angle_helpers() {
        let t = Tolerance::default();
        let a = Length::from_mm(10.0).unwrap();
        let b = Length::from_mm(10.0 + 5e-7).unwrap();
        assert!(t.lengths_eq(a, b));
        assert!(!t.lengths_eq(a, Length::from_mm(10.001).unwrap()));
        assert!(t.is_zero_length(Length::from_mm(1e-7).unwrap()));
        let x = Angle::from_rad(0.1).unwrap();
        let y = Angle::from_rad(0.1 + TAU).unwrap();
        assert!(!t.angles_eq(x, y));
        assert!(t.angles_eq_modulo_turn(x, y));
        assert!(t.angles_eq_modulo_turn(y, x));
        assert!(!t.angles_eq_modulo_turn(x, Angle::from_rad(0.2).unwrap()));
    }
}
