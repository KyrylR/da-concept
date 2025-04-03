//! # Crypto
//!
//! This library provides the foundational mathematical structures for ElGamal encryption.

pub mod encryption;
pub mod errors;
pub mod keypair;
pub mod types;

pub use encryption::{decrypt, encrypt};
pub use errors::CryptoError;
pub use keypair::get_secure_random_bytes;
pub use types::{Ciphertext, PrivateKey, PublicKey};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let private_key = PrivateKey::generate(512);
        let message = "Hello, ElGamal encryption!".to_string();

        let encrypted = encrypt(message.clone(), private_key.clone());
        let decrypted = decrypt(encrypted, private_key).unwrap();

        assert_eq!(message, decrypted);
    }
}
