use num_bigint::BigUint;

/// ElGamal public key
#[derive(Debug, Clone)]
pub struct PublicKey {
    /// Large prime number
    pub p: BigUint,
    /// Generator of the multiplicative group
    pub g: BigUint,
    /// g^x mod p, where x is the private key
    pub h: BigUint,
}

/// ElGamal private key
#[derive(Debug, Clone)]
pub struct PrivateKey {
    /// Secret exponent
    pub x: BigUint,
    /// Associated public key
    pub public_key: PublicKey,
}

/// ElGamal ciphertext
#[derive(Debug, Clone)]
pub struct Ciphertext {
    /// Ephemeral key: g^y
    pub c1: BigUint,
    /// Encrypted message: h^y * m
    pub c2: BigUint,
}
