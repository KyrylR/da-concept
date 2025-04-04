use crate::errors::CryptoError;
use crate::types::{Ciphertext, PrivateKey};

use num_bigint::BigUint;

#[cfg(feature = "tracing")]
use tracing::{debug, error};

/// Encrypt a string using ElGamal encryption.
///
/// The string is split into u32 chunks and each chunk is encrypted.
///
/// # Arguments
///
/// * `input` - The string to encrypt
/// * `private_key` - The private key
///
/// # Returns
///
/// * `Vec<u8>` - The encrypted data
pub fn encrypt<T: AsRef<[u8]>>(input: T, private_key: &PrivateKey) -> Vec<u8> {
    let bytes = input.as_ref();

    #[cfg(feature = "tracing")]
    debug!("Encrypting string with length {}", bytes.len());

    let mut chunks = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut value = 0u32;
        for (i, &byte) in chunk.iter().enumerate() {
            value |= (byte as u32) << (i * 8);
        }
        chunks.push(value);
    }

    #[cfg(feature = "tracing")]
    debug!("Split string into {} chunks", chunks.len());

    let ciphertexts: Vec<Ciphertext> = chunks
        .iter()
        .map(|&chunk| {
            private_key
                .public_key
                .encrypt(&BigUint::from(chunk))
                .unwrap()
        })
        .collect();

    let mut result = Vec::new();

    let count = ciphertexts.len() as u32;
    result.extend_from_slice(&count.to_le_bytes());

    // Store each ciphertext
    for ciphertext in ciphertexts {
        let c1_bytes = ciphertext.c1.to_bytes_le();
        let c2_bytes = ciphertext.c2.to_bytes_le();

        let c1_len = c1_bytes.len() as u32;
        let c2_len = c2_bytes.len() as u32;

        result.extend_from_slice(&c1_len.to_le_bytes());
        result.extend_from_slice(&c1_bytes);
        result.extend_from_slice(&c2_len.to_le_bytes());
        result.extend_from_slice(&c2_bytes);
    }

    #[cfg(feature = "tracing")]
    debug!("Encryption completed: {} bytes of ciphertext", result.len());

    result
}

/// Decrypt a blob of encrypted data using ElGamal decryption.
///
/// # Arguments
///
/// * `data` - The encrypted data
/// * `private_key` - The private key
///
/// # Returns
///
/// * `String` - The decrypted string
pub fn decrypt(data: Vec<u8>, private_key: &PrivateKey) -> Result<String, CryptoError> {
    #[cfg(feature = "tracing")]
    debug!("Decrypting blob with length {}", data.len());

    if data.len() < 4 {
        #[cfg(feature = "tracing")]
        error!("Data is too short to be valid");
        return Err(CryptoError::InvalidCiphertext);
    }

    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    #[cfg(feature = "tracing")]
    debug!("Found {} encrypted chunks", count);

    let mut offset = 4;
    let mut ciphertexts = Vec::with_capacity(count);

    // Read each ciphertext
    for _ in 0..count {
        if offset + 8 > data.len() {
            #[cfg(feature = "tracing")]
            error!("Invalid ciphertext format at offset {}", offset);
            return Err(CryptoError::InvalidCiphertext);
        }

        let c1_len = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if offset + c1_len + 4 > data.len() {
            #[cfg(feature = "tracing")]
            error!("Invalid ciphertext format at offset {}", offset);
            return Err(CryptoError::InvalidCiphertext);
        }

        let c1_bytes = &data[offset..offset + c1_len];
        let c1 = BigUint::from_bytes_le(c1_bytes);
        offset += c1_len;

        // Read c2 length and bytes
        let c2_len = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if offset + c2_len > data.len() {
            #[cfg(feature = "tracing")]
            error!("Invalid ciphertext format at offset {}", offset);
            return Err(CryptoError::InvalidCiphertext);
        }

        let c2_bytes = &data[offset..offset + c2_len];
        let c2 = BigUint::from_bytes_le(c2_bytes);
        offset += c2_len;

        ciphertexts.push(Ciphertext { c1, c2 });
    }

    let mut chunks: Vec<u32> = vec![];
    for ciphertext in ciphertexts {
        chunks.push(private_key.decrypt(&ciphertext)?.to_u32_digits()[0]);
    }

    let mut bytes = Vec::new();
    for chunk in chunks {
        bytes.push((chunk & 0xFF) as u8);
        bytes.push(((chunk >> 8) & 0xFF) as u8);
        bytes.push(((chunk >> 16) & 0xFF) as u8);
        bytes.push(((chunk >> 24) & 0xFF) as u8);
    }

    // Remove padding null bytes from the end
    while bytes.last() == Some(&0) {
        bytes.pop();
    }

    match String::from_utf8(bytes) {
        Ok(string) => {
            #[cfg(feature = "tracing")]
            debug!("Decryption completed successfully");
            Ok(string)
        }
        Err(e) => {
            #[cfg(feature = "tracing")]
            error!("Failed to decode decrypted data: {}", e);
            Err(CryptoError::DecodeDecryptedMessage)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fake::{Fake, Faker};

    use quickcheck::{Arbitrary, Gen, TestResult};
    use quickcheck_macros::quickcheck;

    use rand::distr::Alphanumeric;
    use rand::{Rng, rng};

    fn random_string(len: usize) -> String {
        rng()
            .sample_iter(&Alphanumeric)
            .take(len)
            .map(char::from)
            .collect()
    }

    #[derive(Clone, Debug)]
    struct ArbitraryString(String);

    impl Arbitrary for ArbitraryString {
        fn arbitrary(g: &mut Gen) -> Self {
            let size = g.size();
            let len = usize::arbitrary(g) % (size + 1);
            ArbitraryString(random_string(len))
        }
    }

    #[test]
    fn test_empty_string() {
        let private_key = PrivateKey::generate();
        let empty = String::new();

        let encrypted = encrypt(empty.clone(), &private_key);
        let decrypted = decrypt(encrypted, &private_key).unwrap();

        assert_eq!(empty, decrypted);
    }

    #[test]
    fn test_short_string() {
        let private_key = PrivateKey::generate();
        let message: String = Faker.fake();

        let encrypted = encrypt(message.clone(), &private_key);
        let decrypted = decrypt(encrypted, &private_key).unwrap();

        assert_eq!(message, decrypted);
    }

    #[test]
    fn test_long_string() {
        let private_key = PrivateKey::generate();
        let message = random_string(1000);

        let encrypted = encrypt(message.clone(), &private_key);
        let decrypted = decrypt(encrypted, &private_key).unwrap();

        assert_eq!(message, decrypted);
    }

    #[test]
    fn test_unicode_string() {
        let private_key = PrivateKey::generate();
        let message = "Hello, 世界! 😊 UTF-8 test ñáéíóú".to_string();

        let encrypted = encrypt(message.clone(), &private_key);
        let decrypted = decrypt(encrypted, &private_key).unwrap();

        assert_eq!(message, decrypted);
    }

    #[test]
    fn test_invalid_ciphertext() {
        let private_key = PrivateKey::generate();

        let result = decrypt(vec![1, 2, 3], &private_key);
        assert!(matches!(result, Err(CryptoError::InvalidCiphertext)));

        let mut invalid = vec![0, 0, 0, 1]; // 1 ciphertext
        invalid.extend_from_slice(&[0, 0, 0, 0]); // c1_len = 0

        let result = decrypt(invalid, &private_key);
        assert!(matches!(result, Err(CryptoError::InvalidCiphertext)));
    }

    #[test]
    fn test_different_keys() {
        let key1 = PrivateKey::generate();
        let key2 = PrivateKey::generate();
        let message: String = Faker.fake();

        let encrypted = encrypt(message.clone(), &key1);

        let result = decrypt(encrypted.clone(), &key2);

        if let Ok(decrypted) = result {
            assert_ne!(
                message, decrypted,
                "Decryption with wrong key should not work"
            );
        }

        let correct = decrypt(encrypted, &key1).unwrap();
        assert_eq!(message, correct);
    }

    #[test]
    fn test_corrupted_data() {
        let private_key = PrivateKey::generate();
        let message: String = Faker.fake();

        let mut encrypted = encrypt(message, &private_key);

        if encrypted.len() > 10 {
            encrypted[10] ^= 0xFF; // Flip all bits at position 10

            if let Ok(decrypted) = decrypt(encrypted, &private_key) {
                assert_ne!("Important data", decrypted)
            }
        }
    }

    #[quickcheck]
    fn quickcheck_encrypt_decrypt_roundtrip(s: ArbitraryString) -> TestResult {
        let private_key = PrivateKey::generate();
        let message = s.0;

        let encrypted = encrypt(message.clone(), &private_key);

        match decrypt(encrypted, &private_key) {
            Ok(decrypted) => TestResult::from_bool(message == decrypted),
            Err(_) => TestResult::error("Decryption failed"),
        }
    }

    #[test]
    fn test_with_fake_data() {
        let private_key = PrivateKey::generate();

        for _ in 0..10 {
            let sentence: String = Faker.fake();

            let encrypted = encrypt(sentence.clone(), &private_key);
            let decrypted = decrypt(encrypted, &private_key).unwrap();

            assert_eq!(sentence, decrypted);
        }
    }

    #[test]
    fn test_serialization_format() {
        let private_key = PrivateKey::generate();
        let message = "Test".to_string();

        let encrypted = encrypt(message, &private_key);

        assert!(encrypted.len() >= 4, "Encrypted data too short");

        let count = u32::from_le_bytes([encrypted[0], encrypted[1], encrypted[2], encrypted[3]]);
        assert!(count > 0, "Count should be positive");

        let mut offset = 4;
        for _ in 0..count {
            if offset + 8 > encrypted.len() {
                panic!("Data format invalid: premature end");
            }

            let c1_len = u32::from_le_bytes([
                encrypted[offset],
                encrypted[offset + 1],
                encrypted[offset + 2],
                encrypted[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + c1_len + 4 > encrypted.len() {
                panic!("Data format invalid: c1_len too large");
            }

            offset += c1_len;

            let c2_len = u32::from_le_bytes([
                encrypted[offset],
                encrypted[offset + 1],
                encrypted[offset + 2],
                encrypted[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + c2_len > encrypted.len() {
                panic!("Data format invalid: c2_len too large");
            }

            offset += c2_len;
        }

        assert_eq!(offset, encrypted.len(), "Format invalid: leftover bytes");
    }
}
