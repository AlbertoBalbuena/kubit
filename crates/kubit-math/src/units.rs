//! Length and angle newtypes, unit conversion and display formatting.
//!
//! Values are stored in millimetres / radians. Conversion to other units happens only here, at
//! the edges (parsing user input, formatting for display, file IO).

use core::f64::consts::PI;
use thiserror::Error;

/// Largest accepted length magnitude, in millimetres (1 km).
pub const MAX_LENGTH_MM: f64 = 1.0e6;
/// Largest accepted angle magnitude, in radians (about 1591 turns).
pub const MAX_ANGLE_RAD: f64 = 1.0e4;
/// Millimetres per inch (exact by definition).
pub const MM_PER_INCH: f64 = 25.4;

/// A value failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum UnitError {
    /// The value was NaN or infinite.
    #[error("value is not a finite number")]
    NotFinite,
    /// A length exceeded the documented bound of 1e6 mm.
    #[error("length {mm} mm is outside the allowed range of +-1e6 mm")]
    LengthOutOfBounds {
        /// The offending value, in millimetres.
        mm: f64,
    },
    /// An angle exceeded the documented bound of 1e4 rad.
    #[error("angle {rad} rad is outside the allowed range of +-1e4 rad")]
    AngleOutOfBounds {
        /// The offending value, in radians.
        rad: f64,
    },
}

/// Validates a length in millimetres: finite and `|mm| <= 1e6`.
pub fn validate_length_mm(mm: f64) -> Result<f64, UnitError> {
    if !mm.is_finite() {
        Err(UnitError::NotFinite)
    } else if mm.abs() > MAX_LENGTH_MM {
        Err(UnitError::LengthOutOfBounds { mm })
    } else {
        Ok(mm)
    }
}

/// Validates an angle in radians: finite and `|rad| <= 1e4`.
pub fn validate_angle_rad(rad: f64) -> Result<f64, UnitError> {
    if !rad.is_finite() {
        Err(UnitError::NotFinite)
    } else if rad.abs() > MAX_ANGLE_RAD {
        Err(UnitError::AngleOutOfBounds { rad })
    } else {
        Ok(rad)
    }
}

/// Units of length understood at the edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LengthUnit {
    /// Millimetre (`mm`), the internal unit.
    Millimetre,
    /// Centimetre (`cm`).
    Centimetre,
    /// Metre (`m`).
    Metre,
    /// International inch (`in`, `"`), exactly 25.4 mm.
    Inch,
    /// Foot (`ft`), exactly 12 inches.
    Foot,
}

impl LengthUnit {
    /// Millimetres in one of this unit.
    pub const fn mm_per_unit(self) -> f64 {
        match self {
            Self::Millimetre => 1.0,
            Self::Centimetre => 10.0,
            Self::Metre => 1000.0,
            Self::Inch => MM_PER_INCH,
            Self::Foot => 12.0 * MM_PER_INCH,
        }
    }

    /// Canonical short symbol.
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Millimetre => "mm",
            Self::Centimetre => "cm",
            Self::Metre => "m",
            Self::Inch => "in",
            Self::Foot => "ft",
        }
    }
}

/// Units of angle understood at the edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AngleUnit {
    /// Radian (`rad`), the internal unit.
    Radian,
    /// Degree (`deg`, `°`).
    Degree,
}

impl AngleUnit {
    /// Radians in one of this unit.
    pub const fn rad_per_unit(self) -> f64 {
        match self {
            Self::Radian => 1.0,
            Self::Degree => PI / 180.0,
        }
    }

    /// Canonical short symbol.
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Radian => "rad",
            Self::Degree => "deg",
        }
    }
}

/// Any unit that can follow a number in an expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// A length unit.
    Length(LengthUnit),
    /// An angle unit.
    Angle(AngleUnit),
}

impl Unit {
    /// Parses `mm cm m in " ft deg rad °`.
    pub fn from_symbol(s: &str) -> Option<Self> {
        Some(match s {
            "mm" => Self::Length(LengthUnit::Millimetre),
            "cm" => Self::Length(LengthUnit::Centimetre),
            "m" => Self::Length(LengthUnit::Metre),
            "in" | "\"" => Self::Length(LengthUnit::Inch),
            "ft" => Self::Length(LengthUnit::Foot),
            "deg" | "°" => Self::Angle(AngleUnit::Degree),
            "rad" => Self::Angle(AngleUnit::Radian),
            _ => return None,
        })
    }
}

/// A validated length, stored in millimetres. Always finite and `|mm| <= 1e6`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Length(f64);

impl Length {
    /// The zero length.
    pub const ZERO: Self = Self(0.0);

    /// Creates a length from millimetres.
    pub fn from_mm(mm: f64) -> Result<Self, UnitError> {
        validate_length_mm(mm).map(Self)
    }

    /// Creates a length from a value in `unit`.
    pub fn new(value: f64, unit: LengthUnit) -> Result<Self, UnitError> {
        Self::from_mm(value * unit.mm_per_unit())
    }

    /// The value in millimetres.
    pub const fn mm(self) -> f64 {
        self.0
    }

    /// The value expressed in `unit`.
    pub fn to_unit(self, unit: LengthUnit) -> f64 {
        self.0 / unit.mm_per_unit()
    }
}

/// A validated angle, stored in radians. Always finite and `|rad| <= 1e4`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Angle(f64);

impl Angle {
    /// The zero angle.
    pub const ZERO: Self = Self(0.0);

    /// Creates an angle from radians.
    pub fn from_rad(rad: f64) -> Result<Self, UnitError> {
        validate_angle_rad(rad).map(Self)
    }

    /// Creates an angle from degrees.
    pub fn from_degrees(deg: f64) -> Result<Self, UnitError> {
        Self::new(deg, AngleUnit::Degree)
    }

    /// Creates an angle from a value in `unit`.
    pub fn new(value: f64, unit: AngleUnit) -> Result<Self, UnitError> {
        Self::from_rad(value * unit.rad_per_unit())
    }

    /// The value in radians.
    pub const fn rad(self) -> f64 {
        self.0
    }

    /// The value in degrees.
    pub fn degrees(self) -> f64 {
        self.to_unit(AngleUnit::Degree)
    }

    /// The value expressed in `unit`.
    pub fn to_unit(self, unit: AngleUnit) -> f64 {
        self.0 / unit.rad_per_unit()
    }
}

/// Millimetre display format: fixed decimals, optional trimming of trailing zeros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmFormat {
    /// Digits after the decimal point (clamped to 9).
    pub decimals: u8,
    /// Remove trailing zeros (`12.50` -> `12.5`, `12.00` -> `12`).
    pub trim_zeros: bool,
}

impl Default for MmFormat {
    fn default() -> Self {
        Self {
            decimals: 2,
            trim_zeros: true,
        }
    }
}

impl MmFormat {
    /// Formats `len` as e.g. `12.5 mm`. Negative zero is shown as `0 mm`.
    pub fn format(&self, len: Length) -> String {
        let mut s = format!("{:.*}", usize::from(self.decimals.min(9)), len.mm());
        if self.trim_zeros && s.contains('.') {
            s.truncate(s.trim_end_matches('0').trim_end_matches('.').len());
        }
        if !s.chars().any(|c| c.is_ascii_digit() && c != '0') {
            s = s.trim_start_matches('-').to_owned();
        }
        s.push_str(" mm");
        s
    }
}

/// How a length that is not an exact fraction is rounded to `1/denominator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FractionRounding {
    /// To the nearest fraction (ties away from zero).
    Nearest,
    /// Toward zero (never larger in magnitude than the true value).
    TowardZero,
    /// Away from zero (never smaller in magnitude than the true value).
    AwayFromZero,
}

/// Inch display format with fractions: `3/4"`, `1 1/2"`, `2"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InchFormat {
    /// Largest denominator, clamped to `1..=64`. The result is reduced (`2/4` -> `1/2`).
    pub max_denominator: u8,
    /// Rounding mode for values between two fractions.
    pub rounding: FractionRounding,
}

impl Default for InchFormat {
    fn default() -> Self {
        Self {
            max_denominator: 64,
            rounding: FractionRounding::Nearest,
        }
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl InchFormat {
    /// Formats `len` in inches, e.g. `1 1/2"`. A value that rounds to zero shows as `0"`.
    pub fn format(&self, len: Length) -> String {
        let d = u64::from(self.max_denominator.clamp(1, 64));
        let scaled = len.mm().abs() / MM_PER_INCH * d as f64;
        // Snap values within float noise of a fraction so Floor/Ceil modes stay exact.
        let n = if (scaled - scaled.round()).abs() < 1.0e-9 {
            scaled.round()
        } else {
            match self.rounding {
                FractionRounding::Nearest => scaled.round(),
                FractionRounding::TowardZero => scaled.floor(),
                FractionRounding::AwayFromZero => scaled.ceil(),
            }
        } as u64;
        if n == 0 {
            return "0\"".to_owned();
        }
        let sign = if len.mm() < 0.0 { "-" } else { "" };
        let (whole, rem) = (n / d, n % d);
        let g = gcd(rem, d);
        let (num, den) = (rem / g, d / g);
        match (whole, rem) {
            (w, 0) => format!("{sign}{w}\""),
            (0, _) => format!("{sign}{num}/{den}\""),
            (w, _) => format!("{sign}{w} {num}/{den}\""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mm(v: f64) -> Length {
        Length::from_mm(v).unwrap()
    }

    #[test]
    fn inch_is_exactly_25_4_mm() {
        assert_eq!(Length::new(1.0, LengthUnit::Inch).unwrap().mm(), 25.4);
        assert_eq!(mm(25.4).to_unit(LengthUnit::Inch), 1.0);
        assert_eq!(Length::new(10.0, LengthUnit::Inch).unwrap().mm(), 254.0);
    }

    #[test]
    fn unit_oracles() {
        let cases = [
            (LengthUnit::Millimetre, 1.0),
            (LengthUnit::Centimetre, 10.0),
            (LengthUnit::Metre, 1000.0),
            (LengthUnit::Inch, 25.4),
            (LengthUnit::Foot, 304.8),
        ];
        for (u, f) in cases {
            assert!((Length::new(2.0, u).unwrap().mm() - 2.0 * f).abs() < 1e-9);
            assert!((mm(f).to_unit(u) - 1.0).abs() < 1e-12);
        }
        assert!((Angle::from_degrees(180.0).unwrap().rad() - PI).abs() < 1e-15);
        assert!((Angle::from_rad(PI / 2.0).unwrap().degrees() - 90.0).abs() < 1e-12);
    }

    #[test]
    fn symbols_roundtrip() {
        for u in [
            LengthUnit::Millimetre,
            LengthUnit::Centimetre,
            LengthUnit::Metre,
            LengthUnit::Inch,
            LengthUnit::Foot,
        ] {
            assert_eq!(Unit::from_symbol(u.symbol()), Some(Unit::Length(u)));
        }
        for u in [AngleUnit::Radian, AngleUnit::Degree] {
            assert_eq!(Unit::from_symbol(u.symbol()), Some(Unit::Angle(u)));
        }
        assert_eq!(
            Unit::from_symbol("\""),
            Some(Unit::Length(LengthUnit::Inch))
        );
        assert_eq!(Unit::from_symbol("°"), Some(Unit::Angle(AngleUnit::Degree)));
        assert_eq!(Unit::from_symbol("parsec"), None);
    }

    #[test]
    fn validation_bounds() {
        assert_eq!(Length::from_mm(1.0e6).unwrap().mm(), 1.0e6);
        assert_eq!(
            Length::from_mm(1.0e6 + 1.0),
            Err(UnitError::LengthOutOfBounds { mm: 1.0e6 + 1.0 })
        );
        assert_eq!(Length::from_mm(f64::NAN), Err(UnitError::NotFinite));
        assert_eq!(Length::from_mm(f64::INFINITY), Err(UnitError::NotFinite));
        assert_eq!(
            Length::new(f64::MAX, LengthUnit::Metre),
            Err(UnitError::NotFinite)
        );
        assert!(Angle::from_rad(1.0e4).is_ok());
        assert!(matches!(
            Angle::from_rad(1.0e4 + 1.0),
            Err(UnitError::AngleOutOfBounds { .. })
        ));
        assert_eq!(Angle::from_degrees(f64::NAN), Err(UnitError::NotFinite));
    }

    #[test]
    fn mm_formatting() {
        let f = MmFormat::default();
        assert_eq!(f.format(mm(12.5)), "12.5 mm");
        assert_eq!(f.format(mm(12.0)), "12 mm");
        assert_eq!(f.format(mm(12.345)), "12.35 mm");
        assert_eq!(f.format(mm(-0.001)), "0 mm");
        let fixed = MmFormat {
            decimals: 3,
            trim_zeros: false,
        };
        assert_eq!(fixed.format(mm(12.5)), "12.500 mm");
        assert_eq!(fixed.format(mm(-0.0)), "0.000 mm");
        let wide = MmFormat {
            decimals: 200,
            trim_zeros: false,
        };
        assert_eq!(wide.format(mm(1.0)), "1.000000000 mm");
    }

    #[test]
    fn inch_fraction_formatting() {
        let f = InchFormat::default();
        let inch = |v: f64| Length::new(v, LengthUnit::Inch).unwrap();
        assert_eq!(f.format(inch(0.75)), "3/4\"");
        assert_eq!(f.format(inch(1.5)), "1 1/2\"");
        assert_eq!(f.format(inch(2.0)), "2\"");
        assert_eq!(f.format(inch(0.0)), "0\"");
        assert_eq!(f.format(inch(-0.75)), "-3/4\"");
        assert_eq!(f.format(inch(-1.5)), "-1 1/2\"");
        assert_eq!(f.format(inch(1.0 / 64.0)), "1/64\"");
        assert_eq!(f.format(inch(3.0 + 5.0 / 16.0)), "3 5/16\"");
        assert_eq!(f.format(mm(19.05)), "3/4\"");
    }

    #[test]
    fn inch_rounding_flag() {
        let inch = |v: f64| Length::new(v, LengthUnit::Inch).unwrap();
        let v = inch(0.5 + 0.3 / 64.0); // 30% of the way to 33/64
        let mk = |rounding| InchFormat {
            max_denominator: 64,
            rounding,
        };
        assert_eq!(mk(FractionRounding::Nearest).format(v), "1/2\"");
        assert_eq!(mk(FractionRounding::TowardZero).format(v), "1/2\"");
        assert_eq!(mk(FractionRounding::AwayFromZero).format(v), "33/64\"");
        assert_eq!(
            mk(FractionRounding::AwayFromZero).format(inch(-0.5 - 0.3 / 64.0)),
            "-33/64\""
        );
        // Exact fractions are untouched by directed rounding.
        assert_eq!(mk(FractionRounding::TowardZero).format(inch(0.75)), "3/4\"");
        assert_eq!(
            mk(FractionRounding::AwayFromZero).format(inch(0.75)),
            "3/4\""
        );
        // Tiny values.
        assert_eq!(mk(FractionRounding::Nearest).format(mm(0.01)), "0\"");
        assert_eq!(
            mk(FractionRounding::AwayFromZero).format(mm(0.01)),
            "1/64\""
        );
    }

    #[test]
    fn inch_denominator_limit_and_clamp() {
        let inch = |v: f64| Length::new(v, LengthUnit::Inch).unwrap();
        let f = InchFormat {
            max_denominator: 8,
            rounding: FractionRounding::Nearest,
        };
        assert_eq!(f.format(inch(0.3)), "1/4\"");
        assert_eq!(f.format(inch(0.126)), "1/8\"");
        let zero = InchFormat {
            max_denominator: 0,
            rounding: FractionRounding::Nearest,
        };
        assert_eq!(zero.format(inch(1.4)), "1\"");
        let big = InchFormat {
            max_denominator: 255,
            rounding: FractionRounding::Nearest,
        };
        assert_eq!(big.format(inch(1.0 / 128.0 + 1.0)), "1 1/64\"");
    }
}
