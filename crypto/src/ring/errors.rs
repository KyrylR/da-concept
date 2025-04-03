#[derive(thiserror::Error, Debug)]
pub enum RingError {
    /// Error when trying to find a modular inverse that doesn't exist (gcd(a, k) != 1).
    #[error("NoInverse: {0}")]
    NoInverse(String),
    /// Error performing ops on elements with different moduli or from incompatible ring definitions.
    #[error("RingMismatch: {0}")]
    RingMismatch(String),
    /// Error when creating a ring with an invalid modulus (k <= 1).
    #[error("InvalidModulus: {0}")]
    InvalidModulus(String),
    /// Error when modulus k is too large for table generation.
    #[error("ModulusTooLarge: {0}")]
    ModulusTooLarge(String),
}
