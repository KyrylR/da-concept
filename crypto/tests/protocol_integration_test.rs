use crypto::system::protocol::{
    ProtocolParams, alice_decrypt, alice_prepare, bob_prepare, setup_shared,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn test_full_protocol_run() {
    // --- Parameters ---
    // Ensure m is prime and constraints are met
    let param_a: i64 = 3;
    let param_c: i64 = 5;
    let param_l: i64 = 2;
    let param_m: i64 = 7; // Prime
    let params =
        ProtocolParams::new(param_a, param_c, param_l, param_m).expect("Invalid parameters");

    // Use a fixed seed for reproducible tests
    let seed: u64 = 2024;
    let mut rng = StdRng::seed_from_u64(seed);

    // --- Setup ---
    println!("Running Setup...");
    let (_b_seq, _c_seq, phi, ring_m) = setup_shared(&params, seed).expect("Setup failed");
    println!("Setup Complete.");

    // --- Alice Prepare ---
    let p: usize = 3; // equations
    let q: usize = 3; // variables (make p=q for unique solution likelihood)
    let r_transforms: usize = 2;
    println!("Alice Preparing...");
    let (public_info, alice_state) =
        alice_prepare(&params, &phi, &ring_m, p, q, r_transforms, &mut rng)
            .expect("Alice preparation failed");
    println!("Alice Preparation Complete.");
    println!("Public Info A_bar: {:?}", public_info.l_bar_matrix_gk);
    println!("Public Info B_bar: {:?}", public_info.big_l_bar_matrix_gk);
    println!("Public Info a_bar: {:?}", public_info.big_l_bar_vector_gk);

    // --- Bob Prepare ---
    // Bob chooses a secret message v (size p) in Z_m
    let bob_secret_v_zm = vec![1, 2, 3]; // Example message
    // let bob_secret_v_zm = matrix_ops::random_vector(p, &ring_m, &mut rng); // Or random

    println!("Bob Preparing...");
    let (transmission, _bob_state) = bob_prepare(
        &params,
        &phi,
        &ring_m,
        &public_info,
        &bob_secret_v_zm,
        &mut rng,
    )
    .expect("Bob preparation failed");
    println!("Bob Preparation Complete.");
    println!("Transmission d_bar: {:?}", transmission.d_bar_gk);
    println!("Transmission d1_bar: {:?}", transmission.d1_bar_gk);

    // --- Alice Decrypt ---
    println!("Alice Decrypting...");
    let recovered_v_zm =
        alice_decrypt(&alice_state, &transmission).expect("Alice decryption failed");
    println!("Alice Decryption Complete.");

    // --- Verification ---
    println!("Original message v: {:?}", bob_secret_v_zm);
    println!("Recovered message v: {:?}", recovered_v_zm);
    assert_eq!(
        recovered_v_zm, bob_secret_v_zm,
        "Decrypted message does not match original!"
    );
    println!("SUCCESS: Protocol completed and message verified.");
}

#[test]
fn test_invalid_params() {
    // m not prime
    assert!(ProtocolParams::new(3, 5, 2, 6).is_err());
    // gcd(a, k) != 1
    assert!(ProtocolParams::new(2, 5, 3, 5).is_err()); // k=15, gcd(2,15)=1 - OK
    assert!(ProtocolParams::new(3, 5, 2, 5).is_err()); // k=10, gcd(3,10)=1 - OK
    assert!(ProtocolParams::new(2, 5, 2, 5).is_err()); // k=10, gcd(2,10)=2 - ERR
    // gcd(a, m) != 1
    assert!(ProtocolParams::new(7, 5, 2, 7).is_err()); // m=7, gcd(7,7)=7 - ERR
}

// Add more tests for edge cases, error conditions, different p/q values etc.
