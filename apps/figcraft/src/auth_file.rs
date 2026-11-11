//! The saved ERP login, as ciphertext. The file is not JSON and does not contain the tokens
//! as text. On Windows the bytes are sealed for the current user (DPAPI): another account or
//! another PC cannot open them. Elsewhere the file is AES-256-GCM and the key stays in the
//! system key store.

const MAGIC: &[u8] = b"FCA1";
#[cfg(windows)]
const KIND_DPAPI: u8 = 1;
#[cfg(not(windows))]
const KIND_AES: u8 = 2;
const MAX_PLAIN: usize = 16 * 1024;
const MAX_FILE: usize = 64 * 1024;
const FAIL_SEAL: &str = "couldn't protect the saved login";
const FAIL_OPEN: &str = "couldn't read the saved login";

/// Ciphertext for `plain`. Empty input is refused.
pub fn seal(plain: &str) -> Result<Vec<u8>, String> {
    if plain.is_empty() || plain.len() > MAX_PLAIN {
        return Err(FAIL_SEAL.into());
    }
    #[cfg(windows)]
    {
        seal_dpapi(plain.as_bytes())
    }
    #[cfg(not(windows))]
    {
        let key = login_key()?;
        seal_aes(&key, plain.as_bytes())
    }
}

/// The login JSON, when `bytes` is a file this program sealed.
pub fn open(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < 6 || bytes.len() > MAX_FILE || &bytes[..MAGIC.len()] != MAGIC {
        return Err(FAIL_OPEN.into());
    }
    let plain = match bytes[MAGIC.len()] {
        #[cfg(windows)]
        KIND_DPAPI => open_dpapi(&bytes[MAGIC.len() + 1..])?,
        #[cfg(not(windows))]
        KIND_AES => {
            let key = login_key()?;
            open_aes(&key, &bytes[MAGIC.len() + 1..])?
        }
        _ => return Err(FAIL_OPEN.into()),
    };
    String::from_utf8(plain).map_err(|_| FAIL_OPEN.to_string())
}

#[cfg(windows)]
fn seal_dpapi(plain: &[u8]) -> Result<Vec<u8>, String> {
    let blob = windows_dpapi::encrypt_data(plain, windows_dpapi::Scope::User, Some(b"figcraft-auth-v1")).map_err(|_| FAIL_SEAL.to_string())?;
    let mut out = Vec::with_capacity(MAGIC.len() + 1 + blob.len());
    out.extend_from_slice(MAGIC);
    out.push(KIND_DPAPI);
    out.extend(blob);
    Ok(out)
}

#[cfg(windows)]
fn open_dpapi(blob: &[u8]) -> Result<Vec<u8>, String> {
    windows_dpapi::decrypt_data(blob, windows_dpapi::Scope::User, Some(b"figcraft-auth-v1")).map_err(|_| FAIL_OPEN.to_string())
}

#[cfg(not(windows))]
fn login_key() -> Result<[u8; 32], String> {
    use aes_gcm::aead::{AeadCore, OsRng};
    use aes_gcm::Aes256Gcm;
    let entry = keyring::Entry::new("FigCraft", "auth-key").map_err(|_| FAIL_SEAL.to_string())?;
    match entry.get_secret() {
        Ok(bytes) if bytes.len() == 32 => {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            let generated = Aes256Gcm::generate_key(&mut OsRng);
            entry.set_secret(generated.as_slice()).map_err(|_| FAIL_SEAL.to_string())?;
            let mut key = [0u8; 32];
            key.copy_from_slice(generated.as_slice());
            Ok(key)
        }
        _ => Err(FAIL_SEAL.into()),
    }
}

#[cfg(not(windows))]
fn seal_aes(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher.encrypt(Nonce::from_slice(nonce.as_slice()), plain).map_err(|_| FAIL_SEAL.to_string())?;
    let mut out = Vec::with_capacity(MAGIC.len() + 1 + nonce.len() + ct.len());
    out.extend_from_slice(MAGIC);
    out.push(KIND_AES);
    out.extend_from_slice(nonce.as_slice());
    out.extend(ct);
    Ok(out)
}

#[cfg(not(windows))]
fn open_aes(key: &[u8; 32], body: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    if body.len() < 12 + 16 {
        return Err(FAIL_OPEN.into());
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher.decrypt(Nonce::from_slice(&body[..12]), &body[12..]).map_err(|_| FAIL_OPEN.to_string())
}

#[cfg(test)]
mod tests {
    use super::{open, seal};

    #[test]
    fn a_saved_login_roundtrips_and_the_file_is_not_json() {
        let plain = r#"{"access":"QC-token-value-should-not-appear","refresh":"QC-refresh-value"}"#;
        let sealed = seal(plain).unwrap();
        assert!(sealed.starts_with(b"FCA1"));
        assert!(!sealed.windows(plain.len()).any(|w| w == plain.as_bytes()));
        assert_eq!(open(&sealed).unwrap(), plain);
        assert!(open(plain.as_bytes()).is_err());
        assert!(open(b"FCA1").is_err());
    }
}
