//! Forward-mode automatic differentiation over a handful of local variables.
//!
//! Every residual is written once, generically over [`Real`], and evaluated with
//! [`Dual`] numbers that carry the value and its gradient with respect to the (at most
//! [`MAX_LOCAL`]) variables the constraint touches. The Jacobian is therefore exact,
//! without hand-derived formulas or finite differences.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// Most variables one constraint touches (two lines: four points, eight coordinates,
/// plus one radius).
pub const MAX_LOCAL: usize = 9;

/// A value with its gradient with respect to the local variables.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dual {
    /// Value.
    pub v: f64,
    /// Partial derivatives.
    pub d: [f64; MAX_LOCAL],
}

impl Dual {
    /// A constant.
    pub const fn constant(v: f64) -> Self {
        Self {
            v,
            d: [0.0; MAX_LOCAL],
        }
    }

    /// Local variable number `i` with value `v`.
    pub fn var(v: f64, i: usize) -> Self {
        let mut d = [0.0; MAX_LOCAL];
        if let Some(slot) = d.get_mut(i) {
            *slot = 1.0;
        }
        Self { v, d }
    }

    fn map(self, v: f64, scale: f64) -> Self {
        Self {
            v,
            d: self.d.map(|x| x * scale),
        }
    }

    /// Square root; the derivative is clamped at zero so it stays finite.
    pub fn sqrt(self) -> Self {
        let r = self.v.max(0.0).sqrt();
        self.map(r, 0.5 / r.max(1e-12))
    }

    /// Two-argument arctangent `atan2(self, x)`.
    pub fn atan2(self, x: Self) -> Self {
        let den = (self.v * self.v + x.v * x.v).max(1e-300);
        let mut out = Self::constant(self.v.atan2(x.v));
        for ((o, dy), dx) in out.d.iter_mut().zip(self.d).zip(x.d) {
            *o = (x.v * dy - self.v * dx) / den;
        }
        out
    }
}

impl Add for Dual {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        let mut d = self.d;
        for (a, b) in d.iter_mut().zip(o.d) {
            *a += b;
        }
        Self { v: self.v + o.v, d }
    }
}

impl Sub for Dual {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        self + (-o)
    }
}

impl Neg for Dual {
    type Output = Self;
    fn neg(self) -> Self {
        self.map(-self.v, -1.0)
    }
}

impl Mul for Dual {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let mut d = [0.0; MAX_LOCAL];
        for ((x, a), b) in d.iter_mut().zip(self.d).zip(o.d) {
            *x = a * o.v + b * self.v;
        }
        Self { v: self.v * o.v, d }
    }
}

impl Div for Dual {
    type Output = Self;
    fn div(self, o: Self) -> Self {
        let den = if o.v.abs() < 1e-150 {
            1e-150f64.copysign(o.v)
        } else {
            o.v
        };
        let mut d = [0.0; MAX_LOCAL];
        for ((x, a), b) in d.iter_mut().zip(self.d).zip(o.d) {
            *x = (a * den - b * self.v) / (den * den);
        }
        Self { v: self.v / den, d }
    }
}

impl Mul<f64> for Dual {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        self.map(self.v * k, k)
    }
}

impl Add<f64> for Dual {
    type Output = Self;
    fn add(self, k: f64) -> Self {
        Self {
            v: self.v + k,
            d: self.d,
        }
    }
}

impl Sub<f64> for Dual {
    type Output = Self;
    fn sub(self, k: f64) -> Self {
        self + (-k)
    }
}

/// A 2-vector of duals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V2 {
    /// u.
    pub x: Dual,
    /// v.
    pub y: Dual,
}

impl V2 {
    /// `self − o`.
    pub fn minus(self, o: Self) -> Self {
        Self {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }

    /// Dot product.
    pub fn dot(self, o: Self) -> Dual {
        self.x * o.x + self.y * o.y
    }

    /// z of the cross product.
    pub fn cross(self, o: Self) -> Dual {
        self.x * o.y - self.y * o.x
    }

    /// Length (smooth away from zero).
    pub fn norm(self) -> Dual {
        self.dot(self).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivatives_match_calculus() {
        let x = Dual::var(3.0, 0);
        let y = Dual::var(4.0, 1);
        let r = V2 { x, y }.norm();
        assert!((r.v - 5.0).abs() < 1e-12);
        assert!((r.d[0] - 0.6).abs() < 1e-12 && (r.d[1] - 0.8).abs() < 1e-12);
        let q = x / y;
        assert!((q.d[0] - 0.25).abs() < 1e-12 && (q.d[1] + 3.0 / 16.0).abs() < 1e-12);
        let a = y.atan2(x);
        assert!((a.d[0] + 4.0 / 25.0).abs() < 1e-12 && (a.d[1] - 3.0 / 25.0).abs() < 1e-12);
    }

    #[test]
    fn degenerate_inputs_stay_finite() {
        let z = Dual::var(0.0, 0);
        assert!(z.sqrt().d.iter().all(|d| d.is_finite()));
        assert!((Dual::constant(1.0) / z).d.iter().all(|d| d.is_finite()));
        assert!(z.atan2(z).d.iter().all(|d| d.is_finite()));
    }
}
