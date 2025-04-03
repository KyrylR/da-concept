use crate::errors::CryptoError;
use crate::types::{Ciphertext, PrivateKey, PublicKey};

use std::ops::Rem;

use num_bigint::BigUint;
use num_traits::One;

use tracing::{error, trace};

pub fn get_secure_random_bytes() -> [u8; 32] {
    ring::rand::generate(&ring::rand::SystemRandom::new())
        .unwrap()
        .expose()
}

impl PrivateKey {
    /// Generate a new ElGamal keypair
    pub fn generate(bits: usize) -> Self {
        trace!("Generating new ElGamal keypair with {} bits", bits);

        let p = BigUint::parse_bytes(
            b"21888242871839275222246405745257275088696311157297823662689037894645226208583",
            10,
        )
        .expect("Unable to parse BigUint");

        let g = BigUint::from(2u32);

        let random_biguint = BigUint::from_bytes_be(&get_secure_random_bytes());

        let x = random_biguint % (&p - BigUint::one());

        let h = g.modpow(&x, &p);

        let public_key = PublicKey { p, g, h };

        trace!("ElGamal keypair generated successfully");

        PrivateKey { x, public_key }
    }

    /// Decrypt an ElGamal ciphertext
    pub fn decrypt(&self, ciphertext: &Ciphertext) -> Result<BigUint, CryptoError> {
        trace!("Decrypting ElGamal ciphertext");

        // Compute s = c1^x mod p
        let s = ciphertext.c1.modpow(&self.x, &self.public_key.p);

        // Compute s^(-1) mod p
        let s_inv = s.modpow(
            &(self.public_key.p.clone() - BigUint::from(2u32)),
            &self.public_key.p,
        );

        // Recover message m = c2 * s^(-1) mod p
        let m = (ciphertext.c2.clone() * s_inv).rem(&self.public_key.p);

        Ok(m)
    }
}

impl PublicKey {
    /// Encrypt a message using ElGamal encryption
    pub fn encrypt(&self, message: &BigUint) -> Result<Ciphertext, CryptoError> {
        if message >= &self.p {
            error!("Message is too large: {}", message);
            return Err(CryptoError::InvalidCiphertext);
        }

        trace!("Encrypting message with ElGamal");

        let random_biguint = BigUint::from_bytes_be(&get_secure_random_bytes());

        let y = random_biguint % (&self.p - BigUint::one());

        // Compute c1 = g^y mod p
        let c1 = self.g.modpow(&y, &self.p);

        // Compute s = h^y mod p
        let s = self.h.modpow(&y, &self.p);

        // Compute c2 = m * s mod p
        let c2 = (message * s).rem(&self.p);

        Ok(Ciphertext { c1, c2 })
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
            private_key.public_key.h,
            private_key
                .public_key
                .g
                .modpow(&private_key.x, &private_key.public_key.p)
        );

        assert!(private_key.x > BigUint::zero());
        assert!(private_key.x < private_key.public_key.p.clone() - BigUint::one());
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

        let message = public_key.p.clone() + BigUint::one();
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

        if message_biguint >= public_key.p {
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
}
