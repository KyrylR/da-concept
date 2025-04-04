use num_bigint::BigUint;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// ElGamal public key
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct PublicKey {
    /// Large prime number
    pub prime: BigUint,
    /// Generator of the multiplicative group
    pub generator: BigUint,
    /// g^x mod p, where x is the private key
    pub public_exponent: BigUint,
}

/// ElGamal private key
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct PrivateKey {
    /// Secret exponent
    pub private_key: BigUint,
    /// Associated public key
    pub public_key: PublicKey,
}

/// ElGamal ciphertext
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Ciphertext {
    /// Ephemeral key: g^y
    pub c1: BigUint,
    /// Encrypted message: h^y * m
    pub c2: BigUint,
}
