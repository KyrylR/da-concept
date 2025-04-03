use crate::system::errors::CryptoError;

use std::collections::HashMap;

use rand::Rng;

#[derive(Debug, Clone)] // Added Clone
pub struct Isomorphism {
    m: i64,
    // b_seq: Vec<i64>, // Store maps instead of sequence for direct use
    z_to_g_map: HashMap<i64, i64>,
    g_to_z_map: HashMap<i64, i64>,
}

impl Isomorphism {
    /// Creates the isomorphism mapping based on the defining sequence `b_seq` for G_m.
    /// `b_seq` must be of length `m`, with `b_m = 0` (at index m-1) and `b_1 = 1` (at index 0).
    /// It must define a permutation of {1, ..., m-1} for the non-zero elements.
    pub fn new(b_seq: &[i64], m: i64) -> Result<Self, CryptoError> {
        if m <= 0 {
            return Err(CryptoError::InvalidParameters(
                "Isomorphism modulus m must be positive".to_string(),
            ));
        }
        if b_seq.len() != m as usize {
            return Err(CryptoError::InvalidParameters(format!(
                "Sequence b length ({}) must match m ({})",
                b_seq.len(),
                m
            )));
        }
        // Check b_m = 0 (element at index m-1)
        if b_seq.get((m - 1) as usize) != Some(&0) {
            return Err(CryptoError::InvalidParameters(format!(
                "Last element of b sequence (b_{}) must be 0, found {:?}",
                m,
                b_seq.get((m - 1) as usize)
            )));
        }
        // Check b_1 = 1 (element at index 0) for m > 1
        if m > 1 && b_seq.first() != Some(&1) {
            return Err(CryptoError::InvalidParameters(format!(
                "First element of b sequence (b_1) must be 1 for m>1, found {:?}",
                b_seq.first()
            )));
        }

        let mut z_to_g_map = HashMap::with_capacity(m as usize);
        let mut g_to_z_map = HashMap::with_capacity(m as usize);

        // g(0) = b_m = 0
        z_to_g_map.insert(0, 0);
        g_to_z_map.insert(0, 0);

        // g(i) = b_i for i = 1..m-1 (indices 0 to m-2 in b_seq)
        for i_zm in 1..m {
            // Z_m values
            let b_index = (i_zm - 1) as usize; // b_i is at index i-1
            let g_val = b_seq[b_index];

            // Check validity of g_val from sequence
            if g_val < 0 || g_val >= m {
                return Err(CryptoError::InvalidParameters(format!(
                    "Sequence element b_{} ({}) is out of range [0, {})",
                    i_zm, g_val, m
                )));
            }
            // Check for collisions (ensures b defines permutation on non-zero elements)
            if g_val != 0 && g_to_z_map.contains_key(&g_val) {
                return Err(CryptoError::InvalidParameters(format!(
                    "Sequence b does not define a valid isomorphism: duplicate G_m value {}",
                    g_val
                )));
            }
            // We map Z_m value `i_zm` to G_m value `g_val`
            z_to_g_map.insert(i_zm, g_val);
            g_to_z_map.insert(g_val, i_zm);
        }

        // Final check: ensure all values 0..m-1 are present as keys in g_to_z_map
        // and as values in z_to_g_map (implicitly checked by size).
        if g_to_z_map.len() != m as usize || z_to_g_map.len() != m as usize {
            return Err(CryptoError::InvalidParameters(format!(
                "Sequence b failed to create a full isomorphism map for m={}. Check for duplicates or missing values.",
                m
            )));
        }

        Ok(Isomorphism {
            m,
            // b_seq: b_seq.to_vec(), // Don't store if maps are sufficient
            z_to_g_map,
            g_to_z_map,
        })
    }

    // Map Z_m element to G_m element
    pub fn map_z_to_g(&self, z_val: i64) -> Result<i64, CryptoError> {
        // Normalize z_val to be within [0, m-1] before lookup
        let normalized_z = ((z_val % self.m) + self.m) % self.m;
        self.z_to_g_map.get(&normalized_z).copied().ok_or_else(|| {
            // This should not happen if constructor validated correctly
            CryptoError::IsomorphismError(format!(
                "Value {} not found in Z_m to G_m map (internal error)",
                normalized_z
            ))
        })
    }

    // Map G_m element to Z_m element
    pub fn map_g_to_z(&self, g_val: i64) -> Result<i64, CryptoError> {
        // Ensure g_val is within expected range [0, m-1]
        if g_val < 0 || g_val >= self.m {
            return Err(CryptoError::IsomorphismError(format!(
                "G_m value {} is out of range [0, {})",
                g_val, self.m
            )));
        }
        self.g_to_z_map.get(&g_val).copied().ok_or_else(|| {
            // This should not happen if constructor validated correctly
            CryptoError::IsomorphismError(format!(
                "Value {} not found in G_m to Z_m map (internal error)",
                g_val
            ))
        })
    }

    // Apply map_z_to_g to a vector
    pub fn map_vec_z_to_g(&self, vec_z: &[i64]) -> Result<Vec<i64>, CryptoError> {
        vec_z.iter().map(|&z| self.map_z_to_g(z)).collect()
    }

    // Apply map_g_to_z to a vector
    pub fn map_vec_g_to_z(&self, vec_g: &[i64]) -> Result<Vec<i64>, CryptoError> {
        vec_g.iter().map(|&g| self.map_g_to_z(g)).collect()
    }

    // Apply map_z_to_g to a matrix
    pub fn map_mat_z_to_g(&self, mat_z: &[Vec<i64>]) -> Result<Vec<Vec<i64>>, CryptoError> {
        mat_z.iter().map(|row| self.map_vec_z_to_g(row)).collect()
    }

    // Apply map_g_to_z to a matrix
    pub fn map_mat_g_to_z(&self, mat_g: &[Vec<i64>]) -> Result<Vec<Vec<i64>>, CryptoError> {
        mat_g.iter().map(|row| self.map_vec_g_to_z(row)).collect()
    }
}

// --- Factor Set Correspondent ---

/// Gets a random correspondent in G_k (range [0, k-1]) for a value from G_m (range [0, m-1]).
/// This represents picking an element from the equivalence class { x in G_k | x = g_m_val (mod m) }.
/// `g_m_val` should be in the range [0, m-1].
pub fn get_correspondent(
    g_m_val: i64,
    k: i64,
    m: i64,
    rng: &mut impl Rng,
) -> Result<i64, CryptoError> {
    if m <= 0 || k <= 0 || k < m || k % m != 0 {
        return Err(CryptoError::InvalidParameters(format!(
            "Invalid k={} or m={} for correspondent generation (k>=m, k%m==0 required)",
            k, m
        )));
    }
    if g_m_val < 0 || g_m_val >= m {
        return Err(CryptoError::InvalidParameters(format!(
            "g_m_val ({}) must be in [0, {})",
            g_m_val, m
        )));
    }

    // Number of elements in G_k that map to g_m_val under mod m
    let num_choices = k / m; // Integer division since k % m == 0

    // Choose a random multiplier 'r' such that 0 <= r < num_choices
    let r = rng.gen_range(0..num_choices);

    // Correspondent = g_m_val + r * m
    let correspondent = g_m_val + r * m;

    // Double check bounds (should be guaranteed by logic above)
    if correspondent < 0 || correspondent >= k {
        Err(CryptoError::RandError(format!(
            "Internal error: Generated correspondent {} out of range [0, {})",
            correspondent, k
        )))
    } else {
        Ok(correspondent)
    }
}

/// Maps an element from G_k back to G_m using modulo m.
/// This represents applying the surjection lambda or psi.
/// Result is in the range [0, m-1].
pub fn map_g_k_to_g_m(g_k_val: i64, m: i64) -> Result<i64, CryptoError> {
    if m <= 0 {
        return Err(CryptoError::InvalidParameters(
            "Modulus m must be positive".to_string(),
        ));
    }
    // We don't strictly need k here, just m.
    // Assume g_k_val is a valid value from G_k (e.g., 0 <= g_k_val < k)
    // The modulo operation handles negative inputs correctly.
    Ok(((g_k_val % m) + m) % m)
}

// Apply map_g_k_to_g_m to a vector
pub fn map_vec_g_k_to_g_m(vec_gk: &[i64], m: i64) -> Result<Vec<i64>, CryptoError> {
    vec_gk.iter().map(|&gk| map_g_k_to_g_m(gk, m)).collect()
}

// Apply map_g_k_to_g_m to a matrix
pub fn map_mat_g_k_to_g_m(mat_gk: &[Vec<i64>], m: i64) -> Result<Vec<Vec<i64>>, CryptoError> {
    mat_gk
        .iter()
        .map(|row| map_vec_g_k_to_g_m(row, m))
        .collect()
}
