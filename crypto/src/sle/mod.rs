//! A module for solving systems of linear equations Ax = b
//! over the ring of integers modulo K (Z_K).
//!
//! This implementation uses Gaussian elimination adapted for modular arithmetic.
//! It works best when K is prime. If K is composite, it might fail or give
//! incorrect results if a required modular inverse doesn't exist.

mod errors;
pub use errors::SleError;

use crate::ring::math::Ring;

use std::fmt;

/// Represents the possible outcomes of solving a linear system modulo K.
#[derive(Debug, PartialEq, Eq)]
pub enum Solution {
    /// A unique solution vector x.
    Unique(Vec<i64>),
    /// No solution exists for the system.
    NoSolution,
    /// Infinitely many solutions exist.
    /// Note: For simplicity, this implementation might sometimes report
    /// NoSolution when a required modular inverse doesn't exist for a composite K,
    /// even if infinite solutions might technically exist. A full characterization
    /// for composite K is more complex.
    InfiniteSolutions,
}

impl fmt::Display for Solution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Solution::Unique(x) => write!(f, "Unique solution: {:?}", x),
            Solution::NoSolution => write!(f, "No solution"),
            Solution::InfiniteSolutions => write!(f, "Infinite solutions"),
        }
    }
}

/// Solves the system of linear equations Ax = b modulo K.
///
/// Args:
///   a: The coefficient matrix (n x m).
///   b: The constant vector (n x 1).
///   k: The modulus (must be > 1).
///
/// Returns:
///   A `Result` containing either a `Solution` enum or an `SleError`.
///
/// # Errors
///
/// Returns `SleError` if:
/// - k <= 1
/// - Matrix/vector dimensions are inconsistent
/// - k is not prime (Ring creation fails)
/// - Modular inverse doesn't exist during computation
pub fn solve(a: &[Vec<i64>], b: &[i64], k: u64) -> Result<Solution, SleError> {
    if k <= 1 {
        return Err(SleError::InvalidModulus);
    }

    let n = a.len();
    if n == 0 {
        return Ok(Solution::Unique(Vec::new()));
    }

    let m = a[0].len();

    if b.len() != n {
        return Err(SleError::DimensionMismatch(format!(
            "Matrix A rows ({}) must match vector b length ({})",
            n,
            b.len()
        )));
    }

    for row in a {
        if row.len() != m {
            return Err(SleError::DimensionMismatch(format!(
                "All rows in matrix A must have the same length ({}). Found length {}",
                m,
                row.len()
            )));
        }
    }

    let ring = Ring::try_with(k)?;

    // Create augmented matrix [A | b]
    let mut aug_matrix: Vec<Vec<i64>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut row = a[i].clone();
        row.push(b[i]);

        for val in row.iter_mut() {
            *val = ring.normalize(*val);
        }
        aug_matrix.push(row);
    }

    let mut pivot_row = 0;
    for col in 0..m {
        if pivot_row >= n {
            break; // All rows processed
        }

        // Find pivot: Find a row >= pivot_row with non-zero entry in this column
        let mut pivot_idx = pivot_row;
        while pivot_idx < n && aug_matrix[pivot_idx][col] == 0 {
            pivot_idx += 1;
        }

        if pivot_idx < n {
            // Found a pivot
            // Swap rows if necessary
            if pivot_idx != pivot_row {
                aug_matrix.swap(pivot_row, pivot_idx);
            }

            // Try to make the pivot element 1 by multiplying by its inverse
            let pivot_val = aug_matrix[pivot_row][col];
            match ring.inv(pivot_val) {
                Ok(inv) => {
                    // Multiply the entire pivot row by the inverse
                    for j in col..=m {
                        aug_matrix[pivot_row][j] = ring.mul(aug_matrix[pivot_row][j], inv);
                    }

                    // Eliminate other entries in this column
                    for i in 0..n {
                        if i != pivot_row {
                            let factor = aug_matrix[i][col];
                            if factor != 0 {
                                // R_i = R_i - factor * R_pivot_row
                                for j in col..=m {
                                    let term = ring.mul(factor, aug_matrix[pivot_row][j]);
                                    aug_matrix[i][j] = ring.sub(aug_matrix[i][j], term);
                                }
                            }
                        }
                    }
                    pivot_row += 1;
                }
                Err(_) => {
                    // Cannot find inverse for the pivot.
                    // For prime moduli (enforced by Ring), this should only happen when pivot_val = 0,
                    // which is handled above. For completeness, we report this as NoSolution.
                    return Ok(Solution::NoSolution);
                }
            }
        }
        // If no pivot found in this column (all zeros below pivot_row), move to the next column.
    }

    // --- Check for inconsistencies and determine solution type ---

    // Check for rows like [0 0 ... 0 | c] where c != 0
    for i in pivot_row..n {
        if aug_matrix[i][m] != 0 {
            return Ok(Solution::NoSolution); // Inconsistent system
        }
    }

    // If the number of non-zero rows (rank = pivot_row) is less than the number of variables,
    // and the system is consistent, there are infinite solutions.
    if pivot_row < m {
        return Ok(Solution::InfiniteSolutions);
    }

    // Otherwise, unique solution (rank == number of variables)
    // The solution is in the last column of the pivot rows (in RREF form)
    let mut solution = vec![0i64; m];
    // In RREF, the solution is directly readable if rank == m
    let mut current_col = 0;
    for r in 0..pivot_row {
        // Find the pivot column for this row
        while current_col < m && aug_matrix[r][current_col] == 0 {
            current_col += 1;
        }
        if current_col < m {
            solution[current_col] = aug_matrix[r][m];
            current_col += 1; // Move to next expected pivot column
        }
    }

    Ok(Solution::Unique(solution))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve_unique_prime_k() {
        // System:
        // 2x + 3y = 1 (mod 5)
        // 1x + 2y = 2 (mod 5)
        // Solution: x=1, y=3
        let a = vec![vec![2, 3], vec![1, 2]];
        let b = vec![1, 2];
        let k = 5;
        assert_eq!(solve(&a, &b, k).unwrap(), Solution::Unique(vec![1, 3]));
    }

    #[test]
    fn test_solve_unique_prime_k_3x3() {
        // System (mod 7):
        // 1x + 1y + 1z = 1
        // 0x + 2y + 5z = 2
        // 2x + 0y + 3z = 3
        // Solution: x=6, y=5, z=4
        let a = vec![vec![1, 1, 1], vec![0, 2, 5], vec![2, 0, 3]];
        let b = vec![1, 2, 3];
        let k = 7;
        assert_eq!(solve(&a, &b, k).unwrap(), Solution::Unique(vec![6, 5, 4]));
    }

    #[test]
    fn test_solve_no_solution() {
        // System:
        // 1x + 1y = 1 (mod 4)
        // 2x + 2y = 3 (mod 4)
        let a = vec![vec![1, 1], vec![2, 2]];
        let b = vec![1, 3];
        let k = 4; // Composite modulus
        // This should be an error now since Ring requires prime modulus
        assert!(solve(&a, &b, k).is_err());
    }

    #[test]
    fn test_solve_infinite_solutions() {
        // System:
        // 1x + 1y = 1 (mod 5)
        // 2x + 2y = 2 (mod 5)
        // Dependent equations, infinite solutions
        let a = vec![vec![1, 1], vec![2, 2]];
        let b = vec![1, 2];
        let k = 5; // Prime modulus
        assert_eq!(solve(&a, &b, k).unwrap(), Solution::InfiniteSolutions);
    }

    #[test]
    fn test_invalid_modulus() {
        let a = vec![vec![1, 1]];
        let b = vec![1];
        assert!(matches!(solve(&a, &b, 1), Err(SleError::InvalidModulus)));
    }

    #[test]
    fn test_dimension_mismatch_b() {
        let a = vec![vec![1, 2], vec![3, 4]];
        let b = vec![1];
        let k = 5;
        assert!(matches!(
            solve(&a, &b, k),
            Err(SleError::DimensionMismatch(_))
        ));
    }

    #[test]
    fn test_dimension_mismatch_a() {
        let a = vec![vec![1, 2], vec![3]];
        let b = vec![1, 2];
        let k = 5;
        assert!(matches!(
            solve(&a, &b, k),
            Err(SleError::DimensionMismatch(_))
        ));
    }

    #[test]
    fn test_non_prime_modulus() {
        let a = vec![vec![1, 1]];
        let b = vec![1];
        let k = 4; // Non-prime modulus
        assert!(matches!(solve(&a, &b, k), Err(SleError::RingError(_))));
    }
}
