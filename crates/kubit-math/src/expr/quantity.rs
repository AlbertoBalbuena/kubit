//! Runtime quantities with dimensional analysis.

use super::error::ExprErrorKind;
use super::{MAX_DIM_EXPONENT, MAX_MAGNITUDE};
use crate::units::{Angle, Length, UnitError};
use thiserror::Error;

/// Dimension as integer exponents of length and angle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dim {
    /// Exponent of length (mm).
    pub length: i8,
    /// Exponent of angle (rad).
    pub angle: i8,
}

impl Dim {
    /// A plain number.
    pub const SCALAR: Self = Self {
        length: 0,
        angle: 0,
    };
    /// A length.
    pub const LENGTH: Self = Self {
        length: 1,
        angle: 0,
    };
    /// An angle.
    pub const ANGLE: Self = Self {
        length: 0,
        angle: 1,
    };

    fn bound(v: i16) -> Option<i8> {
        i8::try_from(v).ok().filter(|e| e.abs() <= MAX_DIM_EXPONENT)
    }

    pub(crate) fn checked_add(self, o: Self) -> Option<Self> {
        Some(Self {
            length: Self::bound(i16::from(self.length) + i16::from(o.length))?,
            angle: Self::bound(i16::from(self.angle) + i16::from(o.angle))?,
        })
    }

    pub(crate) fn checked_sub(self, o: Self) -> Option<Self> {
        Some(Self {
            length: Self::bound(i16::from(self.length) - i16::from(o.length))?,
            angle: Self::bound(i16::from(self.angle) - i16::from(o.angle))?,
        })
    }

    pub(crate) fn checked_scale(self, n: i8) -> Option<Self> {
        Some(Self {
            length: Self::bound(i16::from(self.length) * i16::from(n))?,
            angle: Self::bound(i16::from(self.angle) * i16::from(n))?,
        })
    }

    /// Halves both exponents; `None` if either is odd.
    pub(crate) fn halved(self) -> Option<Self> {
        (self.length % 2 == 0 && self.angle % 2 == 0).then_some(Self {
            length: self.length / 2,
            angle: self.angle / 2,
        })
    }
}

impl core::fmt::Display for Dim {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if *self == Self::SCALAR {
            return f.write_str("1");
        }
        let mut first = true;
        for (sym, e) in [("mm", self.length), ("rad", self.angle)] {
            if e == 0 {
                continue;
            }
            if !first {
                f.write_str("*")?;
            }
            first = false;
            if e == 1 {
                f.write_str(sym)?;
            } else {
                write!(f, "{sym}^{e}")?;
            }
        }
        Ok(())
    }
}

/// A value in canonical units (mm^length, rad^angle) with its dimension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    /// Value in canonical units.
    pub value: f64,
    /// Dimension.
    pub dim: Dim,
}

/// Converting a [`Quantity`] to a typed value failed.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum QuantityError {
    /// The dimension is not the one requested.
    #[error("expected dimension {expected}, found {found}")]
    WrongDimension {
        /// Requested dimension.
        expected: Dim,
        /// Actual dimension.
        found: Dim,
    },
    /// The value failed unit validation (non-finite or out of bounds).
    #[error(transparent)]
    Unit(#[from] UnitError),
}

impl Quantity {
    /// A quantity from raw parts (no validation; evaluation validates what it produces).
    pub const fn new(value: f64, dim: Dim) -> Self {
        Self { value, dim }
    }

    /// A plain number.
    pub const fn scalar(value: f64) -> Self {
        Self::new(value, Dim::SCALAR)
    }

    /// A length in millimetres.
    pub const fn length_mm(value: f64) -> Self {
        Self::new(value, Dim::LENGTH)
    }

    /// An angle in radians.
    pub const fn angle_rad(value: f64) -> Self {
        Self::new(value, Dim::ANGLE)
    }

    fn expect(self, expected: Dim) -> Result<f64, QuantityError> {
        if self.dim == expected {
            Ok(self.value)
        } else {
            Err(QuantityError::WrongDimension {
                expected,
                found: self.dim,
            })
        }
    }

    /// As a validated [`Length`].
    pub fn as_length(self) -> Result<Length, QuantityError> {
        Ok(Length::from_mm(self.expect(Dim::LENGTH)?)?)
    }

    /// As a validated [`Angle`].
    pub fn as_angle(self) -> Result<Angle, QuantityError> {
        Ok(Angle::from_rad(self.expect(Dim::ANGLE)?)?)
    }

    /// As a plain number.
    pub fn as_scalar(self) -> Result<f64, QuantityError> {
        self.expect(Dim::SCALAR)
    }

    pub(crate) fn checked(self) -> Result<Self, ExprErrorKind> {
        if self.value.is_nan() {
            Err(ExprErrorKind::NotFinite)
        } else if self.value.abs() > MAX_MAGNITUDE {
            Err(ExprErrorKind::OutOfRange)
        } else {
            Ok(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dim_display() {
        assert_eq!(Dim::SCALAR.to_string(), "1");
        assert_eq!(Dim::LENGTH.to_string(), "mm");
        assert_eq!(Dim::ANGLE.to_string(), "rad");
        assert_eq!(
            Dim {
                length: 2,
                angle: -1
            }
            .to_string(),
            "mm^2*rad^-1"
        );
    }

    #[test]
    fn dim_arithmetic_is_bounded() {
        let l3 = Dim {
            length: 3,
            angle: 0,
        };
        assert_eq!(
            l3.checked_add(l3),
            Some(Dim {
                length: 6,
                angle: 0
            })
        );
        assert_eq!(
            Dim {
                length: 6,
                angle: 0
            }
            .checked_add(Dim::LENGTH),
            None
        );
        assert_eq!(l3.checked_scale(3), None);
        assert_eq!(Dim::LENGTH.checked_sub(Dim::LENGTH), Some(Dim::SCALAR));
        assert_eq!(
            Dim {
                length: 2,
                angle: 0
            }
            .halved(),
            Some(Dim::LENGTH)
        );
        assert_eq!(Dim::LENGTH.halved(), None);
    }

    #[test]
    fn typed_conversions() {
        assert_eq!(Quantity::length_mm(5.0).as_length().unwrap().mm(), 5.0);
        assert_eq!(Quantity::angle_rad(1.0).as_angle().unwrap().rad(), 1.0);
        assert_eq!(Quantity::scalar(2.0).as_scalar().unwrap(), 2.0);
        assert!(matches!(
            Quantity::angle_rad(1.0).as_length(),
            Err(QuantityError::WrongDimension { .. })
        ));
        assert!(matches!(
            Quantity::length_mm(2.0e6).as_length(),
            Err(QuantityError::Unit(UnitError::LengthOutOfBounds { .. }))
        ));
    }
}
