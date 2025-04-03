//! Implementation of ring ops using modular arithmetic.

use super::{RingError, extended_gcd};

/// Represents a finite ring Z_k using modular arithmetic.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Ring {
    modulus: u64,
}

impl Ring {
    /// Create a new Ring with the given modulus, which must be prime.
    pub fn try_with(modulus: u64) -> Result<Self, RingError> {
        if !primal::is_prime(modulus) {
            return Err(RingError::InvalidModulus(format!(
                "Modulus {} is not prime",
                modulus
            )));
        }

        Ok(Ring { modulus })
    }

    pub fn modulus(&self) -> u64 {
        self.modulus
    }

    pub fn normalize(&self, value: i64) -> i64 {
        let modulus_i128 = self.modulus as i128;

        (((value as i128 % modulus_i128) + modulus_i128) % modulus_i128) as i64
    }

    pub fn add(&self, a: i64, b: i64) -> i64 {
        self.normalize(((a as i128 + b as i128) % self.modulus as i128) as _)
    }

    pub fn sub(&self, a: i64, b: i64) -> i64 {
        self.normalize(((a as i128 - b as i128) % self.modulus as i128) as _)
    }

    pub fn mul(&self, a: i64, b: i64) -> i64 {
        self.normalize(((a as i128 * b as i128) % self.modulus as i128) as _)
    }

    pub fn neg(&self, a: i64) -> i64 {
        if a == 0 {
            return 0;
        }

        self.normalize(((-a as i128) % self.modulus as i128) as _)
    }

    pub fn inv(&self, a: i64) -> Result<i64, RingError> {
        let a_norm = self.normalize(a);
        if a_norm == 0 {
            return Ok(0);
        }

        let (g, x, _) = extended_gcd(a_norm, self.modulus as i64);
        if g != 1 {
            return Err(RingError::NoInverse(format!(
                "Modular inverse does not exist for {} mod {}",
                a, self.modulus
            )));
        }

        Ok(self.normalize(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_creation() {
        assert!(Ring::try_with(11).is_ok());
        assert!(Ring::try_with(4).is_err());
    }

    #[test]
    fn test_element_normalization() -> Result<(), RingError> {
        let ring = Ring::try_with(11)?;
        assert_eq!(ring.normalize(5), 5);
        assert_eq!(ring.normalize(16), 5);
        assert_eq!(ring.normalize(-6), 5);
        Ok(())
    }

    #[test]
    fn test_addition() -> Result<(), RingError> {
        let ring = Ring::try_with(11)?;
        assert_eq!(ring.add(5, 8), 2);
        assert_eq!(ring.add(-3, 8), 5);
        Ok(())
    }

    #[test]
    fn test_subtraction() -> Result<(), RingError> {
        let ring = Ring::try_with(11)?;
        assert_eq!(ring.sub(5, 8), 8);
        assert_eq!(ring.sub(8, 5), 3);
        Ok(())
    }

    #[test]
    fn test_multiplication() -> Result<(), RingError> {
        let ring = Ring::try_with(11)?;
        assert_eq!(ring.mul(5, 8), 7);
        assert_eq!(ring.mul(-2, 8), 6);
        Ok(())
    }

    #[test]
    fn test_negation() -> Result<(), RingError> {
        let ring = Ring::try_with(11)?;
        assert_eq!(ring.neg(5), 6);
        assert_eq!(ring.neg(0), 0);
        Ok(())
    }

    #[test]
    fn test_inversion() -> Result<(), RingError> {
        let ring = Ring::try_with(11)?;
        assert_eq!(ring.inv(5)?, 9);
        Ok(())
    }
}
