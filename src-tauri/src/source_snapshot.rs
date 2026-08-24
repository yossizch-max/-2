use crate::error::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct VerifiedSourceSnapshot {
    path: PathBuf,
    sha256: String,
}

impl VerifiedSourceSnapshot {
    pub fn create(source: &Path, expected_sha: &str) -> AppResult<Self> {
        let expected = expected_sha.trim().to_ascii_lowercase();
        if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppError::Validation("invalid source sha256".into()));
        }

        let temp_root = std::env::temp_dir().join("tahrir").join("source-snapshots");
        fs::create_dir_all(&temp_root)?;
        let extension = source.extension().and_then(|x| x.to_str())
            .map(|x| format!(".{x}")).unwrap_or_default();
        let path = temp_root.join(format!("{}{}", Uuid::new_v4(), extension));

        let mut source_file = File::open(source)?;
        let mut snapshot = OpenOptions::new().create_new(true).write(true).open(&path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 1024 * 1024];

        loop {
            let read = source_file.read(&mut buffer)?;
            if read == 0 { break; }
            hasher.update(&buffer[..read]);
            snapshot.write_all(&buffer[..read])?;
        }
        snapshot.flush()?;
        snapshot.sync_all()?;

        let observed = hex::encode(hasher.finalize());
        if observed != expected {
            let _ = fs::remove_file(&path);
            return Err(AppError::SourceShaMismatch);
        }

        Ok(Self { path, sha256: observed })
    }

    pub fn path(&self) -> &Path { &self.path }
    pub fn sha256(&self) -> &str { &self.sha256 }

    pub fn verify_unchanged(&self) -> AppResult<()> {
        if hash_file(&self.path)? != self.sha256 {
            return Err(AppError::SourceSnapshotChanged);
        }
        Ok(())
    }
}

impl Drop for VerifiedSourceSnapshot {
    fn drop(&mut self) { let _ = fs::remove_file(&self.path); }
}

pub fn hash_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 { break; }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_changed_source() {
        let dir = std::env::temp_dir().join(format!("tahrir-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.pdf");
        fs::write(&file, b"A").unwrap();
        let expected = hash_file(&file).unwrap();
        fs::write(&file, b"B").unwrap();
        assert!(matches!(VerifiedSourceSnapshot::create(&file, &expected), Err(AppError::SourceShaMismatch)));
        let _ = fs::remove_dir_all(dir);
    }
}
