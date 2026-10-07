//! Minimal WeAPI request encoding for account and music endpoints.
//!
//! The browser protocol wraps JSON with two AES-CBC passes and RSA encrypts
//! the per-request key. This implementation owns the encoding and exposes only
//! form fields; it does not import code from another client.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use openssl::{
    rsa::{Padding, Rsa},
    symm::{Cipher, encrypt},
};
use serde_json::Value;

use crate::{
    api::protocol,
    error::{BackendError, Result},
};

const IV: &[u8; 16] = b"0102030405060708";
const PRESET_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";
const BASE62: &[u8; 62] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\nMIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDgtQn2JZ34ZC28NWYpAUd98iZ37BUrX/aKzmFbt7clFSs6sXqHauqKWqdtLkF2KexO40H1YTX8z2lSgBBOAxLsvaklV8k4cBFK9snQXE9/DDaFt6Rr7iVZMldczhC0JNgTz+SHXT6CBHuX3e9SdB1Ua44oncaTWz7OBGLbCiK45wIDAQAB\n-----END PUBLIC KEY-----";

pub(crate) fn encode(value: &Value) -> Result<Vec<(String, String)>> {
    let mut random = [0u8; 16];
    rand::fill(&mut random);
    let key: Vec<u8> = random
        .iter()
        .map(|byte| BASE62[(*byte % 62) as usize])
        .collect();
    let fields = encode_with_key(value, &key)?;
    #[cfg(test)]
    TEST_REQUEST_KEYS
        .lock()
        .unwrap()
        .insert(fields[1].1.clone(), key);
    Ok(fields)
}

// Mock servers can inspect real encrypted requests without fixing production randomness.
#[cfg(test)]
static TEST_REQUEST_KEYS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[cfg(test)]
pub(crate) fn decode_test_request(body: &str) -> Value {
    use openssl::symm::decrypt;
    let url = reqwest::Url::parse(&format!("http://127.0.0.1/?{body}")).unwrap();
    let fields = url
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    let key = TEST_REQUEST_KEYS
        .lock()
        .unwrap()
        .remove(&fields["encSecKey"])
        .unwrap();
    let outer = STANDARD.decode(&fields["params"]).unwrap();
    let inner = decrypt(Cipher::aes_128_cbc(), &key, Some(IV), &outer).unwrap();
    let inner = STANDARD.decode(inner).unwrap();
    let plain = decrypt(Cipher::aes_128_cbc(), PRESET_KEY, Some(IV), &inner).unwrap();
    serde_json::from_slice(&plain).unwrap()
}

fn encode_with_key(value: &Value, key: &[u8]) -> Result<Vec<(String, String)>> {
    let text =
        serde_json::to_string(value).map_err(|_| protocol("cannot encode login parameters"))?;
    let first = aes_cbc(&text, PRESET_KEY)?;
    let second = aes_cbc(&STANDARD.encode(first), key)?;
    let reversed: Vec<u8> = key.iter().rev().copied().collect();
    let rsa = Rsa::public_key_from_pem(PUBLIC_KEY.as_bytes())
        .map_err(|_| BackendError::Protocol("invalid login encryption key".into()))?;
    let size = rsa.size() as usize;
    if reversed.len() > size {
        return Err(protocol("login encryption key is too long"));
    }
    let mut padded = vec![0u8; size - reversed.len()];
    padded.extend_from_slice(&reversed);
    let mut encrypted = vec![0u8; size];
    rsa.public_encrypt(&padded, &mut encrypted, Padding::NONE)
        .map_err(|_| protocol("cannot encrypt login key"))?;
    Ok(vec![
        ("params".into(), STANDARD.encode(second)),
        ("encSecKey".into(), hex::encode(encrypted)),
    ])
}

fn aes_cbc(text: &str, key: &[u8]) -> Result<Vec<u8>> {
    aes_cbc_bytes(text.as_bytes(), key)
}

fn aes_cbc_bytes(data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    encrypt(Cipher::aes_128_cbc(), key, Some(IV), data)
        .map_err(|_| protocol("cannot encrypt login parameters"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::symm::decrypt;

    #[test]
    fn produces_two_form_fields_without_plaintext_credentials() {
        let fields = encode(&serde_json::json!({"type":"1", "key":"synthetic-key"})).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0, "params");
        assert_eq!(fields[1].0, "encSecKey");
        assert_ne!(fields[0].1, "synthetic-key");
        assert_eq!(fields[1].1.len(), 256);
        assert!(fields[1].1.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn encrypted_form_round_trips_the_login_parameters() {
        let value = serde_json::json!({"type":"1", "key":"synthetic-key"});
        let fields = encode_with_key(&value, b"0123456789abcdef").unwrap();
        let outer = STANDARD.decode(&fields[0].1).unwrap();
        let inner = decrypt(Cipher::aes_128_cbc(), b"0123456789abcdef", Some(IV), &outer).unwrap();
        let inner = STANDARD.decode(inner).unwrap();
        let json = decrypt(Cipher::aes_128_cbc(), PRESET_KEY, Some(IV), &inner).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&json).unwrap(), value);
    }
}
