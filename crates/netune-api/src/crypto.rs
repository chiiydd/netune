//! API request encryption (linuxapi / eapi).
//!
//! Netease Cloud Music uses two encryption schemes:
//! - linuxapi: AES-128-ECB with a fixed key
//! - eapi: AES-128-ECB + MD5 digest

use aes::Aes128;
use aes::cipher::KeyInit;
use base64::Engine;
use md5::{Digest, Md5};
use netune_core::Result;

// ─── linuxapi ────────────────────────────────────────────────────────────────

const LINUXAPI_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";

/// Encrypt payload using the linuxapi method (AES-128-ECB).
///
/// Used for login endpoints.
pub fn encrypt_linuxapi(data: &str) -> Result<String> {
    let encrypted = aes_ecb_encrypt(data.as_bytes(), LINUXAPI_KEY)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(encrypted))
}

// ─── eapi ────────────────────────────────────────────────────────────────────

const EAPI_KEY: &[u8; 16] = b"e82ckenh8dichen8";

/// Encrypt payload using the eapi method (AES-128-ECB + MD5).
///
/// Used for most API endpoints.
pub fn encrypt_eapi(data: &str, path: &str) -> Result<String> {
    // Build the message: "nobody" + path + "use" + data + "md5forencrypt"
    let message = format!("nobody{path}use{data}md5forencrypt");

    // MD5 digest of the message
    let mut hasher = Md5::new();
    hasher.update(message.as_bytes());
    let digest = hex::encode(hasher.finalize());

    // Final plaintext: data + digest
    let plaintext = format!("{data}{digest}");

    let encrypted = aes_ecb_encrypt(plaintext.as_bytes(), EAPI_KEY)?;
    Ok(hex::encode_upper(encrypted))
}

/// Generate request params for the eapi endpoint.
///
/// Returns a vec of (key, value) pairs to send as form data.
pub fn make_request_params(data: &str, path: &str) -> Result<Vec<(String, String)>> {
    let params = encrypt_eapi(data, path)?;
    Ok(vec![("params".to_string(), params)])
}

// ─── AES-128-ECB helpers ─────────────────────────────────────────────────────

/// PKCS7-padded AES-128-ECB encryption (manual block-by-block).
fn aes_ecb_encrypt(data: &[u8], key: &[u8; 16]) -> Result<Vec<u8>> {
    use aes::cipher::BlockEncrypt;

    let cipher = Aes128::new(key.into());

    // PKCS7 padding
    let block_size = 16;
    let padding_len = block_size - (data.len() % block_size);
    let mut padded = data.to_vec();
    padded.extend(std::iter::repeat(padding_len as u8).take(padding_len));

    // Encrypt each block in-place
    for chunk in padded.chunks_mut(block_size) {
        let block = aes::Block::from_mut_slice(chunk);
        cipher.encrypt_block(block);
    }

    Ok(padded)
}

/// PKCS7-padded AES-128-ECB decryption (manual block-by-block).
#[allow(dead_code)]
fn aes_ecb_decrypt(data: &[u8], key: &[u8; 16]) -> Result<Vec<u8>> {
    use aes::cipher::BlockDecrypt;

    let cipher = Aes128::new(key.into());
    let block_size = 16;

    let mut output = data.to_vec();
    for chunk in output.chunks_mut(block_size) {
        let block = aes::Block::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }

    // Remove PKCS7 padding
    if let Some(&last) = output.last() {
        let pad = last as usize;
        if pad > 0 && pad <= block_size && output.len() >= pad {
            let valid = output.len() - pad;
            output.truncate(valid);
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linuxapi_encrypt() {
        let result = encrypt_linuxapi(r#"{"username":"test","password":"123456"}"#);
        assert!(result.is_ok());
        let encrypted = result.unwrap();
        assert!(!encrypted.is_empty());
    }

    #[test]
    fn test_eapi_encrypt() {
        let result = encrypt_eapi(r#"{"id":123}"#, "/api/song/url");
        assert!(result.is_ok());
        let encrypted = result.unwrap();
        assert!(!encrypted.is_empty());
    }

    #[test]
    fn test_aes_roundtrip() {
        let data = b"Hello, World!";
        let key = b"0123456789abcdef";
        let encrypted = aes_ecb_encrypt(data, key).unwrap();
        let decrypted = aes_ecb_decrypt(&encrypted, key).unwrap();
        assert_eq!(data.to_vec(), decrypted);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let data = r#"{"username":"test","password":"pass123"}"#;
        let encrypted = encrypt_linuxapi(data).unwrap();
        // base64 decode → AES decrypt → original plaintext
        let encrypted_bytes = base64::engine::general_purpose::STANDARD
            .decode(&encrypted)
            .unwrap();
        let decrypted_bytes = aes_ecb_decrypt(&encrypted_bytes, LINUXAPI_KEY).unwrap();
        let decrypted = String::from_utf8(decrypted_bytes).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encrypt_eapi_with_empty_params() {
        let result = encrypt_eapi("", "/api/test");
        assert!(result.is_ok());
        let encrypted = result.unwrap();
        assert!(!encrypted.is_empty());
        // Should be valid hex (uppercase)
        assert!(encrypted.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hex_digest_consistency() {
        use md5::Digest;
        let input = "test input data for md5";
        let mut hasher1 = md5::Md5::new();
        hasher1.update(input.as_bytes());
        let digest1 = hex::encode(hasher1.finalize());

        let mut hasher2 = md5::Md5::new();
        hasher2.update(input.as_bytes());
        let digest2 = hex::encode(hasher2.finalize());

        assert_eq!(digest1, digest2);
        assert_eq!(digest1.len(), 32); // MD5 = 128 bits = 32 hex chars
    }
}
