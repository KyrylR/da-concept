use crate::errors::CryptoError;
use crate::types::{Ciphertext, PrivateKey, PublicKey};

use std::ops::Rem;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use num_bigint::BigUint;
use num_traits::One;

use tracing::{debug, error};

const PRIME: &[u8] =
    b"21888242871839275222246405745257275088696311157297823662689037894645226208583";

pub fn get_secure_random_bytes() -> [u8; 32] {
    ring::rand::generate(&ring::rand::SystemRandom::new())
        .unwrap()
        .expose()
}

impl PrivateKey {
    /// Create a new ElGamal private key from a given private key
    pub fn from(private_key: BigUint) -> Self {
        let prime = BigUint::parse_bytes(PRIME, 10).expect("Unable to parse BigUint");

        let generator = BigUint::from(2u32);
        let public_exponent = generator.modpow(&private_key, &prime);

        let public_key = PublicKey { prime, generator, public_exponent };

        PrivateKey { private_key, public_key }
    }

    /// Generate a new ElGamal keypair
    pub fn generate(bits: usize) -> Self {
        debug!("Generating new ElGamal keypair with {} bits", bits);

        PrivateKey::from(BigUint::from_bytes_be(&get_secure_random_bytes()))
    }

    /// Export the private key as a base64-encoded string
    pub fn get_encoded_private_key(&self) -> String {
        STANDARD.encode(&self.private_key.to_bytes_le())
    }

    /// Decrypt an ElGamal ciphertext
    pub fn decrypt(&self, ciphertext: &Ciphertext) -> Result<BigUint, CryptoError> {
        debug!("Decrypting ElGamal ciphertext");

        // Compute s = c1^x mod p
        let s = ciphertext.c1.modpow(&self.private_key, &self.public_key.prime);

        // Compute s^(-1) mod p
        let s_inv = s.modpow(
            &(self.public_key.prime.clone() - BigUint::from(2u32)),
            &self.public_key.prime,
        );

        // Recover message m = c2 * s^(-1) mod p
        let m = (ciphertext.c2.clone() * s_inv).rem(&self.public_key.prime);

        Ok(m)
    }
}

impl TryFrom<&str> for PrivateKey {
    type Error = CryptoError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let decoded_key = STANDARD.decode(value)?;

        Ok(PrivateKey::from(BigUint::from_bytes_le(&decoded_key)))
    }
}

impl PublicKey {
    /// Encrypt a message using ElGamal encryption
    pub fn encrypt(&self, message: &BigUint) -> Result<Ciphertext, CryptoError> {
        if message >= &self.prime {
            error!("Message is too large: {}", message);
            return Err(CryptoError::InvalidCiphertext);
        }

        debug!("Encrypting message with ElGamal");

        let random_biguint = BigUint::from_bytes_be(&get_secure_random_bytes());

        let y = random_biguint % (&self.prime - BigUint::one());

        // Compute c1 = g^y mod p
        let c1 = self.generator.modpow(&y, &self.prime);

        // Compute s = h^y mod p
        let s = self.public_exponent.modpow(&y, &self.prime);

        // Compute c2 = m * s mod p
        let c2 = (message * s).rem(&self.prime);

        Ok(Ciphertext { c1, c2 })
    }

    /// Export the public key as a base64-encoded string
    pub fn get_encoded_public_key(&self) -> String {
        STANDARD.encode(&self.public_exponent.to_bytes_le())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::Zero;

    use quickcheck::{Arbitrary, Gen, TestResult};
    use quickcheck_macros::quickcheck;

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    struct SmallBigUint(BigUint);

    impl Arbitrary for SmallBigUint {
        fn arbitrary(g: &mut Gen) -> Self {
            let size = g.size();
            let n = u32::arbitrary(g) % (size as u32 + 1);
            SmallBigUint(BigUint::from(n))
        }
    }

    #[test]
    fn test_key_generation() {
        let private_key = PrivateKey::generate(512);

        assert_eq!(
            private_key.public_key.public_exponent,
            private_key
                .public_key
                .generator
                .modpow(&private_key.private_key, &private_key.public_key.prime)
        );

        assert!(private_key.private_key > BigUint::zero());
        assert!(private_key.private_key < private_key.public_key.prime.clone() - BigUint::one());
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let private_key = PrivateKey::generate(512);
        let public_key = private_key.public_key.clone();

        for i in 1..100 {
            let message = BigUint::from(i as u32);
            let ciphertext = public_key.encrypt(&message).unwrap();
            let decrypted = private_key.decrypt(&ciphertext).unwrap();

            assert_eq!(message, decrypted, "Failed on message: {}", i);
        }
    }

    #[test]
    fn test_encrypt_large_message_fails() {
        let private_key = PrivateKey::generate(512);
        let public_key = private_key.public_key.clone();

        let message = public_key.prime.clone() + BigUint::one();
        let result = public_key.encrypt(&message);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CryptoError::InvalidCiphertext
        ));
    }

    #[test]
    fn test_secure_random_bytes() {
        let bytes1 = get_secure_random_bytes();
        let bytes2 = get_secure_random_bytes();

        assert_ne!(bytes1, bytes2, "Secure random bytes should be different");
        assert_eq!(bytes1.len(), 32, "Should generate 32 bytes");
    }

    #[quickcheck]
    fn quickcheck_encrypt_decrypt_preserves_message(message: u32) -> TestResult {
        let private_key = PrivateKey::generate(512);
        let public_key = private_key.public_key.clone();

        let message_biguint = BigUint::from(message);

        if message_biguint >= public_key.prime {
            return TestResult::discard();
        }

        match public_key.encrypt(&message_biguint) {
            Ok(ciphertext) => match private_key.decrypt(&ciphertext) {
                Ok(decrypted) => TestResult::from_bool(message_biguint == decrypted),
                Err(_) => TestResult::error("Decryption failed"),
            },
            Err(_) => TestResult::error("Encryption failed"),
        }
    }

    #[test]
    fn test_different_keys_produce_different_ciphertexts() {
        let key1 = PrivateKey::generate(512);
        let key2 = PrivateKey::generate(512);

        let message = BigUint::from(42u32);

        let ciphertext1 = key1.public_key.encrypt(&message).unwrap();
        let ciphertext2 = key2.public_key.encrypt(&message).unwrap();

        assert_ne!(ciphertext1.c1, ciphertext2.c1);
        assert_ne!(ciphertext1.c2, ciphertext2.c2);
    }

    #[test]
    fn test_export_import_private_key() {
        let key1 = PrivateKey::generate(512);
        let key2 = PrivateKey::try_from(key1.get_encoded_private_key().as_str()).unwrap();

        assert_eq!(key1.private_key, key2.private_key);
    }
}
