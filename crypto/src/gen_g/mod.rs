mod errors;
pub use errors::GenGError;

use crate::helper::gcd;

use rand::prelude::{SeedableRng, SliceRandom, StdRng};

/// Implements the GEN-G(a, c, l, k) algorithm.
///
/// Generates a defining sequence `P` used for constructing ring operation tables.
///
/// # Parameters
/// * `a`: Coefficient 'a' for the linear function f(i) = a*i + c.
/// * `c`: Coefficient 'c' for the linear function f(i) = a*i + c.
/// * `l`: Parameter 'l' from the description, must be > 0 and divide `k`.
/// * `k`: The order (length of the sequence), where k = l*m. Must be > 0.
///
/// # Constraints
/// * `k > 0`
/// * `l > 0` and `k % l == 0`
/// * `gcd(a, k) == 1`
/// * `gcd(a, m) == 1`, where `m = k / l`.
///
/// # Returns
/// * `Ok(Vec<usize>)`: The generated sequence `P` of length `k`.
/// * `Err(GenGError)`: An error if constraints are not met or an internal
///   issue occurs.
///
/// # Algorithm Steps & Notes
/// 1. Generates an initial sequence `b` where `b[i] = (a*i + c) % k`.
/// 2. **Transforms `b` using a seeded shuffle.** The sequence `b` generated in
///    Step 1 is shuffled using a pseudo-random number generator initialized
///    with the provided `seed`. This ensures the transformation is repeatable
///    if the same seed is used.
/// 3. Modifies the shuffled `b` to place `0` at the last position (`b[k-1]`)
///    and `1` at the first position (`b[0]`) by swapping elements.
/// 4. Constructs the final sequence `P` based on the modified `b`.
///
/// # Time Complexity
/// The description states O(k log^2 k) due to multiplications. The shuffle in
/// Step 2 is typically O(k). Other implemented steps are O(k).
pub fn gen_g(a: i64, c: i64, l: i64, k: i64, seed: i64) -> Result<Vec<i64>, GenGError> {
    if k <= 0 {
        return Err(GenGError::KMustBePositive);
    }
    if l == 0 || k % l != 0 {
        return Err(GenGError::InvalidLValue);
    }

    let m = k / l; // Safe because l > 0 and k % l == 0

    // Check GCD constraints
    let gcd_ak = gcd(a, k);
    if gcd_ak != 1 {
        return Err(GenGError::GcdAKConstraintNotMet(gcd_ak));
    }
    let gcd_am = gcd(a, m);
    if gcd_am != 1 {
        return Err(GenGError::GcdAMConstraintNotMet(gcd_am));
    }

    let mut b: Vec<i64> = vec![0; k as usize];

    // Step 1: Initial sequence generation: b[i] = (a*i + c) mod k
    for i in 0..k as usize {
        let term_a_i = (a % k)
            .checked_mul(i as i64 % k)
            .ok_or(GenGError::CalculationOverflow)?;
        b[i] = (term_a_i % k + (c % k)) % k;
    }

    // Step 2: Transformation (Deterministic, based on Example 1)
    if k > 1 {
        b.shuffle(&mut StdRng::seed_from_u64(seed as u64));
    }

    // Step 3: Fix positions of 0 and 1 using the "find index first" approach
    // Find index of 0 in the *shuffled* sequence
    let idx0 = b
        .iter()
        .position(|&x| x == 0)
        .ok_or(GenGError::ValueZeroNotFound)?;
    if (idx0 as i64) != k - 1 {
        b.swap(idx0, (k - 1) as usize);
    }

    // Find index of 1 *after* the potential swap involving 0
    let idx1 = b
        .iter()
        .position(|&x| x == 1)
        .ok_or(GenGError::ValueOneNotFound)?;
    if idx1 != 0 {
        b.swap(idx1, 0);
    }
    // Now `b` holds the result of step 3

    // Step 4: Construct P from the final sequence b
    let mut p: Vec<i64> = vec![0; k as usize];

    if k == 1 {
        // After step 1, b=[0]. Step 2 shuffle does nothing. Step 3 does nothing.
        if b[0] == 0 {
            p[0] = 0;
        } else {
            // Should be unreachable for k=1
            return Err(GenGError::ConstructionFailed);
        }
    } else {
        p[0] = b[0];
        for i in 0..=(k as usize - 3) {
            let index_p = b[i];
            if index_p >= k {
                return Err(GenGError::IndexOutOfBounds(index_p));
            }
            p[index_p as usize] = b[i + 1];
        }
        let index_p_last: i64 = b[k as usize - 2];
        if index_p_last >= k {
            return Err(GenGError::IndexOutOfBounds(index_p_last));
        }
        p[index_p_last as usize] = 0;
    }

    Ok(p)
}
#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SEED: i64 = 42;

    #[test]
    fn test_example_1_with_seed() {
        let k = 6;
        let a = 1;
        let c = 4;
        let l = 2;

        let p_expected = vec![1, 4, 3, 5, 2, 0];

        let result = match gen_g(a, c, l, k, TEST_SEED) {
            Ok(result) => result,
            Err(e) => panic!("Expected Ok, got Err: {:?}", e),
        };

        assert_eq!(result, p_expected);
    }

    #[test]
    fn test_generates_same_p_with_same_seed() {
        let p1 = gen_g(3, 1, 2, 4, TEST_SEED).unwrap();
        let p2 = gen_g(3, 1, 2, 4, TEST_SEED).unwrap();

        assert_eq!(p1, p2);
    }

    #[test]
    fn test_generates_different_p_with_different_seed() {
        let p1 = gen_g(3, 1, 2, 4, TEST_SEED).unwrap();
        let p2 = gen_g(3, 1, 2, 4, TEST_SEED + 1).unwrap();

        assert_ne!(p1, p2);
    }

    #[test]
    fn test_gcd_ak_constraint_error() {
        let result = gen_g(2, 4, 2, 6, TEST_SEED);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), GenGError::GcdAKConstraintNotMet(2));
    }

    #[test]
    fn test_invalid_l_value() {
        let result = gen_g(1, 1, 0, 6, TEST_SEED);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), GenGError::InvalidLValue);

        let result = gen_g(1, 1, 4, 6, TEST_SEED);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), GenGError::InvalidLValue);
    }

    #[test]
    fn test_k0_error() {
        let result = gen_g(1, 1, 1, 0, TEST_SEED);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), GenGError::KMustBePositive);
    }
}
