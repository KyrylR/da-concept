#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    #[error("Failed to generate private key")]
    PrimeSetup,

    #[error("Invalid private key")]
    InvalidPrivateKey,

    #[error("Invalid ciphertext")]
    InvalidCiphertext,

    #[error("Failed to decode string")]
    DecodingError,

    #[error("Random number generation error")]
    RandomGenerationError,
}
