use crate::helper::gcd;
use crate::system::errors::CryptoError;
use crate::system::matrix_ops::{
    mat_inv, mat_mul, mat_vec_mul, random_invertible_matrix, random_vector, vec_add, vec_sub,
};
use crate::system::protocol_helpers::{
    Isomorphism, get_correspondent, map_mat_g_k_to_g_m, map_vec_g_k_to_g_m,
};
use crate::{Ring, gen_g, sle};

use rand::Rng;

#[derive(Clone, Debug)]
pub struct ProtocolParams {
    pub a: i64,
    pub c: i64,
    pub l: i64,
    pub m: i64, // Must be prime
    pub k: i64, // k = l * m
}

impl ProtocolParams {
    pub fn new(a: i64, c: i64, l: i64, m: i64) -> Result<Self, CryptoError> {
        let k = l.checked_mul(m).ok_or(CryptoError::Overflow)?;
        if k <= 0 || l <= 0 || m <= 1 {
            return Err(CryptoError::InvalidParameters(
                "k, l must be positive, m must be > 1".to_string(),
            ));
        }
        if k % l != 0 {
            // Should be guaranteed by k = l*m, but check anyway
            return Err(CryptoError::InvalidParameters(
                "k must be a multiple of l".to_string(),
            ));
        }
        if !primal::is_prime(m as u64) {
            return Err(CryptoError::InvalidParameters(format!(
                "m ({}) must be prime",
                m
            )));
        }
        if gcd(a.abs(), k.abs()) != 1 {
            return Err(CryptoError::InvalidParameters(format!(
                "gcd(a, k) = gcd({}, {}) != 1",
                a, k
            )));
        }
        if gcd(a.abs(), m.abs()) != 1 {
            return Err(CryptoError::InvalidParameters(format!(
                "gcd(a, m) = gcd({}, {}) != 1",
                a, m
            )));
        }

        Ok(ProtocolParams { a, c, l, m, k })
    }
}

#[derive(Debug)] // Avoid Clone unless necessary, contains secrets
pub struct AliceState {
    params: ProtocolParams,
    ring_m: Ring,
    phi: Isomorphism,
    // Secrets for decryption (in Z_m)
    b_inv_matrices_zm: Vec<Vec<Vec<i64>>>, // B_r^{-1}, ..., B_1^{-1}
    a_vectors_zm: Vec<Vec<i64>>,           // a_1, ..., a_{r+1}
}

#[derive(Debug)] // Avoid Clone unless necessary
pub struct BobState {
    params: ProtocolParams,
    ring_m: Ring,
    phi: Isomorphism,
    // Bob's chosen values (in Z_m)
    secret_v_zm: Vec<i64>,
    a_bar_zm: Vec<i64>,
    x_bar_zm: Vec<i64>, // Solution to l_hat(x) = v
}

#[derive(Debug, Clone)]
pub struct PublicInfo {
    // Correspondents in G_k
    pub l_bar_matrix_gk: Vec<Vec<i64>>,     // A_bar
    pub l_bar_vector_gk: Vec<i64>,          // Constant part of l_bar (usually 0)
    pub big_l_bar_matrix_gk: Vec<Vec<i64>>, // B_bar
    pub big_l_bar_vector_gk: Vec<i64>,      // a_bar (constant part of L_bar)
}

#[derive(Debug, Clone)]
pub struct Transmission {
    // Correspondents in G_k
    pub d_bar_gk: Vec<i64>,
    pub d1_bar_gk: Vec<i64>,
}

// --- Protocol Functions ---

/// Performs the initial setup shared between Alice and Bob.
/// Generates the defining sequences and the isomorphism.
pub fn setup_shared(
    params: &ProtocolParams,
    seed: u64, // Use u64 seed for StdRng
) -> Result<(Vec<i64>, Vec<i64>, Isomorphism, Ring), CryptoError> {
    // Generate defining sequence b for G_m (length m)
    // Use gen_g with k'=m, l'=1. Requires gcd(a, m)=1 (checked in params.new)
    let b_seq =
        gen_g::gen_g(params.a, params.c, 1, params.m, seed as i64).map_err(CryptoError::GenG)?; // Map error

    // Generate defining sequence c for G_k (length k)
    // Use original l, k. Requires gcd(a, k)=1 (checked in params.new)
    // Use a different seed or the same? Text implies same params. Let's use different.
    let c_seq = gen_g::gen_g(params.a, params.c, params.l, params.k, (seed + 1) as i64)
        .map_err(CryptoError::GenG)?; // Map error

    // Create the ring Z_m
    let ring_m = Ring::try_with(params.m as u64).map_err(CryptoError::Ring)?; // Map error

    // Create the isomorphism Z_m <-> G_m based on b_seq
    let phi = Isomorphism::new(&b_seq, params.m)?;

    Ok((b_seq, c_seq, phi, ring_m))
}

/// Alice prepares the public information.
/// p: number of equations, q: number of variables, r_transforms: number of affine steps
pub fn alice_prepare(
    params: &ProtocolParams,
    phi: &Isomorphism,
    ring_m: &Ring,
    p: usize, // equations
    q: usize, // variables
    r_transforms: usize,
    rng: &mut impl Rng,
) -> Result<(PublicInfo, AliceState), CryptoError> {
    // a) Define system l(x) = Ax in G_m (coefficients 0..m-1)
    let mut matrix_a_gm = vec![vec![0; q]; p];
    for r_idx in 0..p {
        for c_idx in 0..q {
            matrix_a_gm[r_idx][c_idx] = rng.gen_range(0..params.m);
        }
    }
    // Ensure A is chosen such that Ax=v likely has a unique solution later.
    // For prime m, this often means p >= q and A has full rank q.
    // If p < q, unique solutions are generally not possible.
    // If p=q, A needs to be invertible.
    // We'll proceed but note this constraint for Bob's step.
    if p < q {
        println!(
            "Warning: p < q ({} < {}), system Ax=v may not have unique solutions.",
            p, q
        );
    }

    // b) Transform l(x) to L(x) = Bx + a in G_m
    let mut current_mat_gm = matrix_a_gm.clone();
    let mut current_vec_gm = vec![0; p]; // Start with zero vector offset

    let mut b_inv_matrices_gm: Vec<Vec<Vec<i64>>> = Vec::with_capacity(r_transforms);
    let mut a_vectors_gm: Vec<Vec<i64>> = Vec::with_capacity(r_transforms + 1);

    for _i in 0..r_transforms {
        let b_i_gm = random_invertible_matrix(p, ring_m, rng)?;
        let b_i_inv_gm = mat_inv(&b_i_gm, ring_m)?; // Keep inverse for later
        let a_i_gm = random_vector(p, ring_m, rng);

        // Apply transformation: B_i * (current_system + a_i)
        // current_system = current_mat * x + current_vec
        // current_system + a_i = current_mat * x + (current_vec + a_i)
        current_vec_gm = vec_add(&current_vec_gm, &a_i_gm, ring_m)?;
        // B_i * ( ... ) = (B_i * current_mat) * x + (B_i * current_vec)
        current_mat_gm = mat_mul(&b_i_gm, &current_mat_gm, ring_m)?;
        current_vec_gm = mat_vec_mul(&b_i_gm, &current_vec_gm, ring_m)?;

        b_inv_matrices_gm.push(b_i_inv_gm); // Store inverse B_i^{-1}
        a_vectors_gm.push(a_i_gm); // Store a_i
    }
    // Final vector a_{r+1}
    let a_final_gm = random_vector(p, ring_m, rng);
    current_vec_gm = vec_add(&current_vec_gm, &a_final_gm, ring_m)?;
    a_vectors_gm.push(a_final_gm); // Store a_{r+1}

    // Resulting system L(x) = Bx + a in G_m
    let matrix_b_gm = current_mat_gm;
    let vector_a_gm = current_vec_gm;

    // Store Alice's secrets (needs mapping to Z_m for decryption)
    // Map B_inv matrices and a vectors from G_m to Z_m.
    let b_inv_matrices_zm: Vec<Vec<Vec<i64>>> = b_inv_matrices_gm
        .iter()
        .map(|mat_g| phi.map_mat_g_to_z(mat_g))
        .collect::<Result<_, _>>()?;
    let a_vectors_zm: Vec<Vec<i64>> = a_vectors_gm
        .iter()
        .map(|vec_g| phi.map_vec_g_to_z(vec_g))
        .collect::<Result<_, _>>()?;

    // Reverse B_inv for decryption order: B_r^{-1}, ..., B_1^{-1}
    let mut final_b_inv_zm = b_inv_matrices_zm;
    final_b_inv_zm.reverse();

    // c) Replace coefficients with correspondents from G_k / lambda (using mod m)
    // System l(x) = Ax (constant part is zero)
    let mut matrix_a_bar_gk = vec![vec![0; q]; p];
    for r_idx in 0..p {
        for c_idx in 0..q {
            matrix_a_bar_gk[r_idx][c_idx] =
                get_correspondent(matrix_a_gm[r_idx][c_idx], params.k, params.m, rng)?;
        }
    }
    let vector_l_const_gk = vec![0; p]; // Constant part of l(x) is 0

    // System L(x) = Bx + a
    let mut matrix_b_bar_gk = vec![vec![0; q]; p];
    for r_idx in 0..p {
        for c_idx in 0..q {
            matrix_b_bar_gk[r_idx][c_idx] =
                get_correspondent(matrix_b_gm[r_idx][c_idx], params.k, params.m, rng)?;
        }
    }
    let mut vector_a_bar_gk = vec![0; p];
    for r_idx in 0..p {
        vector_a_bar_gk[r_idx] = get_correspondent(vector_a_gm[r_idx], params.k, params.m, rng)?;
    }

    let public_info = PublicInfo {
        l_bar_matrix_gk: matrix_a_bar_gk,
        l_bar_vector_gk: vector_l_const_gk, // Usually zero
        big_l_bar_matrix_gk: matrix_b_bar_gk,
        big_l_bar_vector_gk: vector_a_bar_gk,
    };

    let alice_state = AliceState {
        params: params.clone(),
        ring_m: *ring_m,                   // Clone ring if needed, or pass reference
        phi: phi.clone(),                  // Clone isomorphism
        b_inv_matrices_zm: final_b_inv_zm, // Already reversed
        a_vectors_zm,
    };

    Ok((public_info, alice_state))
}

/// Bob prepares the transmission data for his secret message v.
pub fn bob_prepare(
    params: &ProtocolParams,
    phi: &Isomorphism,
    ring_m: &Ring,
    public_info: &PublicInfo,
    secret_message_v_zm: &[i64], // Bob's message in Z_m (size p)
    rng: &mut impl Rng,
) -> Result<(Transmission, BobState), CryptoError> {
    let p = public_info.big_l_bar_vector_gk.len(); // Infer p
    let q = if p > 0 {
        public_info.big_l_bar_matrix_gk[0].len()
    } else {
        0
    }; // Infer q

    if secret_message_v_zm.len() != p {
        return Err(CryptoError::DimensionMismatch(format!(
            "Secret message length ({}) must match number of equations p ({})",
            secret_message_v_zm.len(),
            p
        )));
    }

    // a) Receive l_bar, L_bar. Map coefficients back G_k -> G_m -> Z_m.
    // Map G_k -> G_m (using lambda = mod m)
    let matrix_a_prime_gm = map_mat_g_k_to_g_m(&public_info.l_bar_matrix_gk, params.m)?;
    let matrix_b_prime_gm = map_mat_g_k_to_g_m(&public_info.big_l_bar_matrix_gk, params.m)?;
    let vector_a_prime_gm = map_vec_g_k_to_g_m(&public_info.big_l_bar_vector_gk, params.m)?;

    // Map G_m -> Z_m (using phi_inv) to get hat systems
    let matrix_a_hat_zm = phi.map_mat_g_to_z(&matrix_a_prime_gm)?;
    let matrix_b_hat_zm = phi.map_mat_g_to_z(&matrix_b_prime_gm)?;
    let vector_a_hat_zm = phi.map_vec_g_to_z(&vector_a_prime_gm)?;

    // Bob chooses a random vector a_bar in Z_m (size q)
    let bob_a_bar_zm = random_vector(q, ring_m, rng);

    // b) Solve l_hat(x) = v for x_bar in Z_m
    // l_hat(x) = A_hat * x
    let solution = sle::solve(&matrix_a_hat_zm, secret_message_v_zm, params.m as u64)
        .map_err(CryptoError::Sle)?; // Map error

    let bob_x_bar_zm = match solution {
        sle::Solution::Unique(x) => x,
        _ => {
            // If no unique solution, Bob cannot proceed with this message/system.
            // This might happen if p < q or A_hat is singular.
            return Err(CryptoError::SleNoUniqueSolution);
        }
    };

    // Compute d = l_hat(a_bar) in Z_m
    let bob_d_zm = mat_vec_mul(&matrix_a_hat_zm, &bob_a_bar_zm, ring_m)?;

    // Compute d1 = L_hat(x_bar + a_bar) in Z_m
    // L_hat(y) = B_hat * y + a_hat
    let x_plus_a_zm = vec_add(&bob_x_bar_zm, &bob_a_bar_zm, ring_m)?;
    let b_times_x_plus_a = mat_vec_mul(&matrix_b_hat_zm, &x_plus_a_zm, ring_m)?;
    let bob_d1_zm = vec_add(&b_times_x_plus_a, &vector_a_hat_zm, ring_m)?;

    // c) Map d, d1 (Z_m) to G_m using phi.
    let bob_d_gm = phi.map_vec_z_to_g(&bob_d_zm)?;
    let bob_d1_gm = phi.map_vec_z_to_g(&bob_d1_zm)?;

    // Replace with correspondents from G_k / psi (using mod m)
    let mut bob_d_bar_gk = vec![0; p];
    let mut bob_d1_bar_gk = vec![0; p];
    for i in 0..p {
        bob_d_bar_gk[i] = get_correspondent(bob_d_gm[i], params.k, params.m, rng)?;
        bob_d1_bar_gk[i] = get_correspondent(bob_d1_gm[i], params.k, params.m, rng)?;
    }

    let transmission = Transmission {
        d_bar_gk: bob_d_bar_gk,
        d1_bar_gk: bob_d1_bar_gk,
    };

    let bob_state = BobState {
        params: params.clone(),
        ring_m: *ring_m,
        phi: phi.clone(),
        secret_v_zm: secret_message_v_zm.to_vec(),
        a_bar_zm: bob_a_bar_zm,
        x_bar_zm: bob_x_bar_zm,
    };

    Ok((transmission, bob_state))
}

/// Alice decrypts the received transmission to recover Bob's message v.
pub fn alice_decrypt(
    alice_state: &AliceState,
    transmission: &Transmission,
) -> Result<Vec<i64>, CryptoError> {
    let p = alice_state.a_vectors_zm[0].len(); // Infer p from vector size
    let r_transforms = alice_state.b_inv_matrices_zm.len(); // Number of B_inv matrices

    if transmission.d_bar_gk.len() != p || transmission.d1_bar_gk.len() != p {
        return Err(CryptoError::DimensionMismatch(
            "Received transmission vectors have incorrect length".to_string(),
        ));
    }

    // a) Receive (d_bar, d1_bar). Map G_k -> G_m (using psi = mod m). Map G_m -> Z_m (using phi_inv).
    // G_k -> G_m
    let d_prime_gm = map_vec_g_k_to_g_m(&transmission.d_bar_gk, alice_state.params.m)?;
    let d1_prime_gm = map_vec_g_k_to_g_m(&transmission.d1_bar_gk, alice_state.params.m)?;

    // G_m -> Z_m
    let alice_d_hat_zm = alice_state.phi.map_vec_g_to_z(&d_prime_gm)?;
    let alice_d1_hat_zm = alice_state.phi.map_vec_g_to_z(&d1_prime_gm)?;

    // b) Recover v using the inverse transformations and d_hat.
    // Alice has d1_hat = L_hat(x_bar + a_bar)
    // Alice has d_hat = l_hat(a_bar)
    // Alice has secrets: b_inv_matrices_zm (B_r^{-1}, ..., B_1^{-1})
    //                   a_vectors_zm (a_1, ..., a_{r+1})

    // Reverse the transformation on d1_hat to get l_hat(x_bar + a_bar)
    // L_hat(y) = B_r(...B_1(l_hat(y) + a_1)... + a_r) + a_{r+1}

    let mut current_val_zm = alice_d1_hat_zm.clone();

    // Step 1: Subtract a_{r+1} (last element of a_vectors_zm)
    current_val_zm = vec_sub(
        &current_val_zm,
        &alice_state.a_vectors_zm[r_transforms],
        &alice_state.ring_m,
    )?;

    // Apply B_i^{-1} and subtract a_i for i = r down to 1
    for i in (0..r_transforms).rev() {
        // B_{i+1}^{-1} is at index (r_transforms - 1 - i) in b_inv_matrices_zm
        let b_inv_zm = &alice_state.b_inv_matrices_zm[r_transforms - 1 - i];
        current_val_zm = mat_vec_mul(b_inv_zm, &current_val_zm, &alice_state.ring_m)?;

        // a_{i+1} is at index i in a_vectors_zm
        current_val_zm = vec_sub(
            &current_val_zm,
            &alice_state.a_vectors_zm[i],
            &alice_state.ring_m,
        )?;
    }

    // The result should be l_hat(x_bar + a_bar)
    let l_hat_x_plus_a_zm = current_val_zm;

    // Recover v = l_hat(x_bar) = l_hat(x_bar + a_bar) - l_hat(a_bar)
    // We have l_hat(x_bar + a_bar) and alice_d_hat_zm = l_hat(a_bar)
    let recovered_v_zm = vec_sub(&l_hat_x_plus_a_zm, &alice_d_hat_zm, &alice_state.ring_m)?;

    Ok(recovered_v_zm)
}
