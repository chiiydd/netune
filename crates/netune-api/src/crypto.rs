//! API request encryption (linuxapi / eapi / weapi).
//!
//! Netease Cloud Music uses three encryption schemes:
//! - linuxapi: AES-128-ECB with a fixed key
//! - eapi: AES-128-ECB + MD5 digest
//! - weapi: double AES-128-CBC + RSA (used for QR login)

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
    padded.extend(std::iter::repeat_n(padding_len as u8, padding_len));

    // Encrypt each block in-place
    for chunk in padded.chunks_mut(block_size) {
        let block = aes::Block::from_mut_slice(chunk);
        cipher.encrypt_block(block);
    }

    Ok(padded)
}

// ─── AES-128-CBC helper ──────────────────────────────────────────────────────

/// PKCS7-padded AES-128-CBC encryption.
fn aes_cbc_encrypt(data: &[u8], key: &[u8; 16], iv: &[u8; 16]) -> Result<Vec<u8>> {
    use aes::cipher::{BlockEncryptMut, KeyIvInit};

    type Aes128CbcEnc = cbc::Encryptor<Aes128>;

    let mut encryptor = Aes128CbcEnc::new_from_slices(key, iv)
        .map_err(|e| netune_core::NetuneError::Crypto(e.to_string()))?;

    // PKCS7 padding
    let block_size = 16;
    let padding_len = block_size - (data.len() % block_size);
    let mut padded = data.to_vec();
    padded.extend(std::iter::repeat_n(padding_len as u8, padding_len));

    // Encrypt in-place using CBC mode
    for chunk in padded.chunks_mut(block_size) {
        let block = aes::Block::from_mut_slice(chunk);
        encryptor.encrypt_block_mut(block);
    }

    Ok(padded)
}

// ─── weapi ────────────────────────────────────────────────────────────────────

const WEAPI_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";

/// Encrypt payload using the weapi method (double AES-128-CBC + RSA).
///
/// Returns `(params, encSecKey)` for form-data submission.
pub fn weapi_encrypt(data: &serde_json::Value) -> Result<(String, String)> {
    let plaintext = serde_json::to_string(data)
        .map_err(|e| netune_core::NetuneError::Crypto(e.to_string()))?;

    // Generate random 16-char alphanumeric second key
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf)
        .map_err(|e| netune_core::NetuneError::Crypto(e.to_string()))?;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let second_key_bytes: [u8; 16] = std::array::from_fn(|i| CHARSET[(buf[i] as usize) % CHARSET.len()]);

    // Fixed IV: 16 zero bytes
    let iv = [0u8; 16];

    // First AES-CBC: plaintext with WEAPI_KEY
    let encrypted1 = aes_cbc_encrypt(plaintext.as_bytes(), WEAPI_KEY, &iv)?;

    // Second AES-CBC: first result with random key
    let encrypted2 = aes_cbc_encrypt(&encrypted1, &second_key_bytes, &iv)?;

    // Base64 encode → params
    let params = base64::engine::general_purpose::STANDARD.encode(&encrypted2);

    // RSA encrypt the second key → encSecKey (hex)
    let enc_sec_key = rsa_encrypt_key(&second_key_bytes)?;

    Ok((params, enc_sec_key))
}

/// RSA-encrypt a 16-byte key using Netease's public key (PKCS#1 v1.5).
fn rsa_encrypt_key(key: &[u8]) -> Result<String> {
    use rsa::pkcs1v15::Pkcs1v15Encrypt;
    use rsa::BigUint;
    use rsa::RsaPublicKey;

    // Netease Cloud Music RSA public key (1024-bit)
    let modulus_bytes = hex::decode(
        "00e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a5aa76d2e417629ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685b7a46bee255932575cce10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7",
    ).map_err(|e| netune_core::NetuneError::Crypto(format!("hex decode error: {e}")))?;
    let modulus = BigUint::from_bytes_be(&modulus_bytes);
    let exponent = BigUint::from(65537u32);

    let pubkey = RsaPublicKey::new(modulus, exponent)
        .map_err(|e| netune_core::NetuneError::Crypto(format!("RSA key error: {e}")))?;

    let padding = Pkcs1v15Encrypt;
    let encrypted = pubkey
        .encrypt(&mut rsa::rand_core::OsRng, padding, key)
        .map_err(|e| netune_core::NetuneError::Crypto(format!("RSA encrypt error: {e}")))?;

    Ok(hex::encode(encrypted))
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

    #[test]
    fn test_weapi_encrypt() {
        let data = serde_json::json!({"type": 1, "noCheckToken": true});
        let result = weapi_encrypt(&data);
        assert!(result.is_ok());
        let (params, enc_sec_key) = result.unwrap();
        assert!(!params.is_empty());
        assert!(!enc_sec_key.is_empty());
        // encSecKey should be valid hex (256 bytes RSA-1024 output = 512 hex chars)
        assert!(enc_sec_key.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(enc_sec_key.len(), 256); // 128 bytes = 256 hex chars
    }

    #[test]
    fn test_weapi_encrypt_unique() {
        let data = serde_json::json!({"type": 1});
        let (params1, key1) = weapi_encrypt(&data).unwrap();
        let (params2, key2) = weapi_encrypt(&data).unwrap();
        // Each call should produce different results (random second key)
        assert_ne!(params1, params2);
        assert_ne!(key1, key2);
    }
}
