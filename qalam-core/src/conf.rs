//! Confidence values with deterministic lattice operations.
//!
//! `Conf` is a `NotNan<f32>` constrained to `[0, 1]`. The lattice operations
//! (AND, OR) are formally specified and stable across versions; changes are
//! semver-major. See `DESIGN.md` §4.3.

use ordered_float::NotNan;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A confidence value in `[0, 1]`, NaN-free, totally ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Conf(NotNan<f32>);

impl Conf {
    /// The minimum confidence value: 0.0.
    pub fn zero() -> Self {
        Conf(NotNan::new(0.0).expect("0.0 is not NaN"))
    }

    /// The maximum confidence value: 1.0.
    pub fn one() -> Self {
        Conf(NotNan::new(1.0).expect("1.0 is not NaN"))
    }

    /// Construct a `Conf` from an `f32`, returning `None` if the value is
    /// outside `[0, 1]` or NaN.
    pub fn new(x: f32) -> Option<Self> {
        if x.is_nan() || !(0.0..=1.0).contains(&x) {
            None
        } else {
            Some(Conf(NotNan::new(x).expect("non-NaN by check")))
        }
    }

    /// Construct a `Conf`, clamping to `[0, 1]`. NaN becomes 0.
    pub fn clamp(x: f32) -> Self {
        if x.is_nan() {
            Self::zero()
        } else {
            Conf(NotNan::new(x.clamp(0.0, 1.0)).expect("non-NaN by check"))
        }
    }

    /// The underlying `f32`.
    pub fn value(self) -> f32 {
        self.0.into_inner()
    }

    /// AND-combination: `a * b`. Used when evidence must agree (e.g. clitic +
    /// stem must both apply).
    ///
    /// Identity-preserving: `and(x, one) == x` and `and(one, x) == x` exactly.
    /// This short-circuits the `1.0 * x` round-trip which is exact in IEEE 754
    /// for f32 — but it makes the algebraic property explicit in the code.
    pub fn and(self, other: Conf) -> Conf {
        let one = Conf::one();
        if other == one {
            return self;
        }
        if self == one {
            return other;
        }
        Conf::clamp(self.value() * other.value())
    }

    /// OR-combination (deterministic noisy-or): `1 - (1-a)(1-b)`. Used when
    /// alternatives compete (e.g. multiple morph analyses).
    ///
    /// Identity-preserving: `or(x, zero) == x` and `or(zero, x) == x` exactly.
    /// The short-circuit is necessary, not cosmetic: `1.0 - (1.0 - x)` does
    /// not round-trip exactly in f32 (e.g. 0.42 -> 0.41999996), and we want
    /// the identity property to hold for algebraic reasoning.
    pub fn or(self, other: Conf) -> Conf {
        let zero = Conf::zero();
        if other == zero {
            return self;
        }
        if self == zero {
            return other;
        }
        Conf::clamp(1.0 - (1.0 - self.value()) * (1.0 - other.value()))
    }
}

impl fmt::Display for Conf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}", self.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_in_range() {
        assert_eq!(Conf::new(0.5).unwrap().value(), 0.5);
        assert!(Conf::new(-0.1).is_none());
        assert!(Conf::new(1.1).is_none());
        assert!(Conf::new(f32::NAN).is_none());
    }

    #[test]
    fn clamp_handles_out_of_range_and_nan() {
        assert_eq!(Conf::clamp(-1.0), Conf::zero());
        assert_eq!(Conf::clamp(2.0), Conf::one());
        assert_eq!(Conf::clamp(f32::NAN), Conf::zero());
    }

    #[test]
    fn and_combination() {
        let a = Conf::new(0.5).unwrap();
        let b = Conf::new(0.4).unwrap();
        assert!((a.and(b).value() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn or_combination() {
        let a = Conf::new(0.5).unwrap();
        let b = Conf::new(0.5).unwrap();
        assert!((a.or(b).value() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn and_is_commutative() {
        let a = Conf::new(0.3).unwrap();
        let b = Conf::new(0.7).unwrap();
        assert_eq!(a.and(b), b.and(a));
    }

    #[test]
    fn or_is_commutative() {
        let a = Conf::new(0.3).unwrap();
        let b = Conf::new(0.7).unwrap();
        assert_eq!(a.or(b), b.or(a));
    }

    #[test]
    fn and_with_one_is_identity() {
        let a = Conf::new(0.42).unwrap();
        assert_eq!(a.and(Conf::one()), a);
    }

    #[test]
    fn or_with_zero_is_identity() {
        let a = Conf::new(0.42).unwrap();
        assert_eq!(a.or(Conf::zero()), a);
    }

    #[test]
    fn total_ordering() {
        let mut xs = vec![
            Conf::new(0.5).unwrap(),
            Conf::new(0.1).unwrap(),
            Conf::new(0.9).unwrap(),
        ];
        xs.sort();
        assert_eq!(
            xs,
            vec![
                Conf::new(0.1).unwrap(),
                Conf::new(0.5).unwrap(),
                Conf::new(0.9).unwrap(),
            ]
        );
    }
}
