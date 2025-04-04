use base64::DecodeError;

#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    #[error("Failed to generate private key")]
    PrimeSetup,
    #[error("Invalid private key")]
    InvalidPrivateKey,
    #[error("Invalid ciphertext")]
    InvalidCiphertext,
    #[error("Failed to decode string: {0}")]
    Base64Decoding(#[from] DecodeError),
    #[error("Random number generation error")]
    RandomGenerationError,
    #[error("Failed to decode decrypted message")]
    DecodeDecryptedMessage
}
