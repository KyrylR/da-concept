use crate::ring::Ring;

use crate::system::errors::CryptoError;
use rand::Rng;

pub fn vec_add(v1: &[i64], v2: &[i64], ring: &Ring) -> Result<Vec<i64>, CryptoError> {
    if v1.len() != v2.len() {
        return Err(CryptoError::DimensionMismatch(
            "Vectors must have the same length for addition".to_string(),
        ));
    }
    Ok(v1
        .iter()
        .zip(v2.iter())
        .map(|(&a, &b)| ring.add(a, b))
        .collect())
}

pub fn vec_sub(v1: &[i64], v2: &[i64], ring: &Ring) -> Result<Vec<i64>, CryptoError> {
    if v1.len() != v2.len() {
        return Err(CryptoError::DimensionMismatch(
            "Vectors must have the same length for subtraction".to_string(),
        ));
    }
    Ok(v1
        .iter()
        .zip(v2.iter())
        .map(|(&a, &b)| ring.sub(a, b))
        .collect())
}

pub fn mat_vec_mul(mat: &[Vec<i64>], vec: &[i64], ring: &Ring) -> Result<Vec<i64>, CryptoError> {
    if mat.is_empty() {
        if vec.is_empty() {
            return Ok(Vec::new()); // 0x0 matrix * empty vector = empty vector
        } else {
            return Err(CryptoError::DimensionMismatch(
                "Cannot multiply empty matrix by non-empty vector".to_string(),
            ));
        }
    }
    let p = mat.len(); // rows in mat
    let q = mat[0].len(); // cols in mat
    if vec.len() != q {
        return Err(CryptoError::DimensionMismatch(format!(
            "Matrix columns ({}) must match vector length ({})",
            q,
            vec.len()
        )));
    }

    let mut result = vec![0; p];
    for i in 0..p {
        if mat[i].len() != q {
            return Err(CryptoError::DimensionMismatch(format!(
                "Matrix row {} has length {}, expected {}",
                i,
                mat[i].len(),
                q
            )));
        }
        let mut sum = 0;
        for j in 0..q {
            // Use ring operations
            let term = ring.mul(mat[i][j], vec[j]);
            sum = ring.add(sum, term);
        }
        result[i] = sum;
    }
    Ok(result)
}

pub fn mat_mul(
    mat1: &[Vec<i64>],
    mat2: &[Vec<i64>],
    ring: &Ring,
) -> Result<Vec<Vec<i64>>, CryptoError> {
    if mat1.is_empty() || mat2.is_empty() {
        // Define behavior for empty matrices, e.g., return empty or error
        // If one is Px0 and other is 0xQ, result is PxQ zeros?
        // Let's return error for simplicity unless specific behavior is needed.
        return Err(CryptoError::DimensionMismatch(
            "Multiplication with empty matrix is not supported/defined here".to_string(),
        ));
    }
    let p1 = mat1.len();
    let q1 = mat1[0].len();
    let p2 = mat2.len();
    let q2 = mat2[0].len();

    if q1 != p2 {
        return Err(CryptoError::DimensionMismatch(format!(
            "Matrix 1 columns ({}) must match Matrix 2 rows ({})",
            q1, p2
        )));
    }

    // Check consistency of dimensions within matrices
    for r in mat1 {
        if r.len() != q1 {
            return Err(CryptoError::DimensionMismatch(
                "Inconsistent rows in mat1".to_string(),
            ));
        }
    }
    for r in mat2 {
        if r.len() != q2 {
            return Err(CryptoError::DimensionMismatch(
                "Inconsistent rows in mat2".to_string(),
            ));
        }
    }

    let mut result = vec![vec![0; q2]; p1];
    for i in 0..p1 {
        for j in 0..q2 {
            let mut sum = 0;
            for k in 0..q1 {
                // q1 == p2
                // Use ring operations
                let term = ring.mul(mat1[i][k], mat2[k][j]);
                sum = ring.add(sum, term);
            }
            result[i][j] = sum;
        }
    }
    Ok(result)
}

// --- Matrix Inversion (mod m) using Gaussian Elimination ---
pub fn mat_inv(mat: &[Vec<i64>], ring: &Ring) -> Result<Vec<Vec<i64>>, CryptoError> {
    let n = mat.len();
    if n == 0 {
        return Ok(Vec::new()); // Inverse of empty matrix is empty matrix
    }
    // Check if square
    if mat.iter().any(|row| row.len() != n) {
        return Err(CryptoError::DimensionMismatch(
            "Matrix must be square for inversion".to_string(),
        ));
    }

    let modulus = ring.modulus() as i64; // Cast needed for modulo ops if using i64

    // Create augmented matrix [mat | I]
    let mut aug_matrix = vec![vec![0; 2 * n]; n];
    for i in 0..n {
        for j in 0..n {
            // Ensure elements are normalized before starting
            aug_matrix[i][j] = ring.normalize(mat[i][j]);
        }
        aug_matrix[i][i + n] = 1; // Identity part
    }

    // Gaussian elimination to get [I | mat_inv]
    for i in 0..n {
        // Target column i
        // Find pivot row >= i
        let mut pivot_row_idx = i;
        while pivot_row_idx < n && aug_matrix[pivot_row_idx][i] == 0 {
            pivot_row_idx += 1;
        }

        if pivot_row_idx == n {
            // No pivot found in this column below row i-1, matrix is singular
            return Err(CryptoError::MatrixSingular);
        }

        // Swap rows to bring pivot to row i
        aug_matrix.swap(i, pivot_row_idx);

        // Make pivot element 1
        let pivot_val = aug_matrix[i][i];
        // Get multiplicative inverse using ring.inv
        let inv = ring.inv(pivot_val).map_err(CryptoError::Ring)?;
        // If inv doesn't exist (e.g., gcd(pivot_val, modulus) != 1), ring.inv should return Err.
        // This implicitly handles singularity for non-prime moduli if ring.inv checks gcd.
        // Since our ring requires prime modulus, inv exists unless pivot_val is 0 (handled above).

        // Multiply the entire pivot row (row i) by the inverse
        for j in i..(2 * n) {
            aug_matrix[i][j] = ring.mul(aug_matrix[i][j], inv);
        }

        // Eliminate other entries in the current column (column i)
        for row_idx in 0..n {
            if row_idx != i {
                let factor = aug_matrix[row_idx][i]; // Element to eliminate
                if factor != 0 {
                    // R_row_idx = R_row_idx - factor * R_i
                    for col_idx in i..(2 * n) {
                        let term_to_sub = ring.mul(factor, aug_matrix[i][col_idx]);
                        aug_matrix[row_idx][col_idx] =
                            ring.sub(aug_matrix[row_idx][col_idx], term_to_sub);
                    }
                }
            }
        }
    } // End Gaussian elimination loop

    // Extract inverse matrix from the right side of augmented matrix
    let mut inv_matrix = vec![vec![0; n]; n];
    for i in 0..n {
        for j in 0..n {
            inv_matrix[i][j] = aug_matrix[i][j + n];
        }
    }

    Ok(inv_matrix)
}

// --- Generate Random Invertible Matrix ---
pub fn random_invertible_matrix(
    size: usize,
    ring: &Ring,
    rng: &mut impl Rng,
) -> Result<Vec<Vec<i64>>, CryptoError> {
    if size == 0 {
        return Ok(Vec::new());
    }
    // Try generating matrices until one is invertible
    // Add a limit to prevent infinite loops in unlikely scenarios
    for _attempt in 0..100 {
        let mut mat = vec![vec![0; size]; size];
        for i in 0..size {
            for j in 0..size {
                // Generate values in [0, modulus-1]
                mat[i][j] = rng.gen_range(0..ring.modulus()) as i64;
            }
        }
        // Check if invertible by trying to compute the inverse
        if mat_inv(&mat, ring).is_ok() {
            return Ok(mat);
        }
        // Retry if singular
    }
    Err(CryptoError::SetupError(format!(
        "Failed to generate invertible matrix of size {} after 100 attempts",
        size
    )))
}

pub fn random_vector(size: usize, ring: &Ring, rng: &mut impl Rng) -> Vec<i64> {
    (0..size)
        .map(|_| rng.random_range(0..ring.modulus()) as i64)
        .collect()
}
