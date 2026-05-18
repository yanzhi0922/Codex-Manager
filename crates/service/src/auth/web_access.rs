use crate::app_settings::{
    get_persisted_app_setting, normalize_optional_text, save_persisted_app_setting,
    APP_SETTING_WEB_ACCESS_PASSWORD_HASH_KEY,
};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use serde_json::Value;
use sha2::{Digest, Sha256};

const WEB_PASSWORD_ARGON2_M_COST_KIB: u32 = 19_456;
const WEB_PASSWORD_ARGON2_T_COST: u32 = 2;
const WEB_PASSWORD_ARGON2_P_COST: u32 = 1;

enum PasswordVerifyResult {
    MatchCurrent,
    MatchLegacy,
    NoMatch,
}

pub fn current_web_access_password_hash() -> Option<String> {
    get_persisted_app_setting(APP_SETTING_WEB_ACCESS_PASSWORD_HASH_KEY)
}

pub fn web_access_password_configured() -> bool {
    current_web_access_password_hash().is_some()
}

pub fn set_web_access_password(password: Option<&str>) -> Result<bool, String> {
    match normalize_optional_text(password) {
        Some(value) => {
            let hashed = hash_web_access_password(&value)?;
            save_persisted_app_setting(APP_SETTING_WEB_ACCESS_PASSWORD_HASH_KEY, Some(&hashed))?;
            Ok(true)
        }
        None => {
            save_persisted_app_setting(APP_SETTING_WEB_ACCESS_PASSWORD_HASH_KEY, Some(""))?;
            Ok(false)
        }
    }
}

pub fn web_auth_status_value() -> Result<Value, String> {
    Ok(serde_json::json!({
        "passwordConfigured": web_access_password_configured(),
    }))
}

pub fn verify_web_access_password(password: &str) -> bool {
    let Some(stored_hash) = current_web_access_password_hash() else {
        return true;
    };
    match verify_password_hash(password, &stored_hash) {
        PasswordVerifyResult::MatchCurrent => true,
        PasswordVerifyResult::MatchLegacy => {
            if let Ok(upgraded_hash) = hash_web_access_password(password) {
                let _ = save_persisted_app_setting(
                    APP_SETTING_WEB_ACCESS_PASSWORD_HASH_KEY,
                    Some(&upgraded_hash),
                );
            }
            true
        }
        PasswordVerifyResult::NoMatch => false,
    }
}

pub fn build_web_access_session_token(password_hash: &str, rpc_token: &str) -> String {
    hex_sha256(format!("codexmanager-web-auth-session:{password_hash}:{rpc_token}").as_bytes())
}

fn hash_web_access_password(password: &str) -> Result<String, String> {
    let params = Params::new(
        WEB_PASSWORD_ARGON2_M_COST_KIB,
        WEB_PASSWORD_ARGON2_T_COST,
        WEB_PASSWORD_ARGON2_P_COST,
        None,
    )
    .map_err(|err| format!("argon2 params invalid: {err}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| format!("argon2 hash failed: {err}"))
}

fn verify_password_hash(password: &str, stored_hash: &str) -> PasswordVerifyResult {
    if verify_argon2_password_hash(password, stored_hash) {
        return PasswordVerifyResult::MatchCurrent;
    }
    if verify_legacy_sha256_password_hash(password, stored_hash) {
        return PasswordVerifyResult::MatchLegacy;
    }
    PasswordVerifyResult::NoMatch
}

fn verify_argon2_password_hash(password: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn verify_legacy_sha256_password_hash(password: &str, stored_hash: &str) -> bool {
    let mut parts = stored_hash.split('$');
    let Some(kind) = parts.next() else {
        return false;
    };
    let Some(salt_hex) = parts.next() else {
        return false;
    };
    let Some(expected_hash) = parts.next() else {
        return false;
    };
    if kind != "sha256" || parts.next().is_some() {
        return false;
    }
    super::rpc::constant_time_eq(
        hex_sha256(format!("{salt_hex}:{password}").as_bytes()).as_bytes(),
        expected_hash.as_bytes(),
    )
}

fn hex_sha256(bytes: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes.as_ref());
    let digest = hasher.finalize();
    hex_encode(digest.as_slice())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_web_access_password_uses_argon2id_format() {
        let hash = hash_web_access_password("test-password").expect("hash");
        assert!(hash.starts_with("$argon2id$"));
    }

    #[test]
    fn verify_password_hash_accepts_argon2_hash() {
        let hash = hash_web_access_password("test-password").expect("hash");
        let result = verify_password_hash("test-password", &hash);
        assert!(matches!(result, PasswordVerifyResult::MatchCurrent));
    }

    #[test]
    fn verify_password_hash_accepts_legacy_sha256_hash() {
        let salt_hex = "00112233445566778899aabbccddeeff";
        let digest = hex_sha256(format!("{salt_hex}:{}", "legacy-password").as_bytes());
        let hash = format!("sha256${salt_hex}${digest}");

        let result = verify_password_hash("legacy-password", &hash);
        assert!(matches!(result, PasswordVerifyResult::MatchLegacy));
    }

    #[test]
    fn verify_password_hash_rejects_wrong_password() {
        let hash = hash_web_access_password("right-password").expect("hash");
        let result = verify_password_hash("wrong-password", &hash);
        assert!(matches!(result, PasswordVerifyResult::NoMatch));
    }
}
