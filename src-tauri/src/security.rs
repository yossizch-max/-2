use crate::error::{AppError, AppResult};
use keyring::Entry;

const SERVICE: &str = "TAHRIR";
const DB_KEY_USER: &str = "database-key-v1";
const AI_KEY_PREFIX: &str = "ai-key:";

pub fn load_or_create_db_key(db_exists: bool) -> AppResult<String> {
    let entry = Entry::new(SERVICE, DB_KEY_USER).map_err(|e| AppError::Keyring(e.to_string()))?;
    match entry.get_password() {
        Ok(value) => Ok(value),
        Err(_) if db_exists => Err(AppError::RecoveryRequired),
        Err(_) => {
            let mut key = [0u8; 32];
            getrandom::getrandom(&mut key)
                .map_err(|e| AppError::Validation(format!("OS RNG failed: {e}")))?;
            let encoded = hex::encode(key);
            entry.set_password(&encoded).map_err(|e| AppError::Keyring(e.to_string()))?;
            Ok(encoded)
        }
    }
}

pub fn set_ai_secret(profile_id: &str, secret: &str) -> AppResult<()> {
    let entry = Entry::new(SERVICE, &format!("{AI_KEY_PREFIX}{profile_id}"))
        .map_err(|e| AppError::Keyring(e.to_string()))?;
    entry.set_password(secret).map_err(|e| AppError::Keyring(e.to_string()))
}

pub fn get_ai_secret(profile_id: &str) -> AppResult<String> {
    let entry = Entry::new(SERVICE, &format!("{AI_KEY_PREFIX}{profile_id}"))
        .map_err(|e| AppError::Keyring(e.to_string()))?;
    entry.get_password().map_err(|e| AppError::Keyring(e.to_string()))
}
