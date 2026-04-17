//! SHA256 verification helpers for model download integrity.
//!
//! Extracted from `model/mod.rs`. Stateless functions operating on file paths.

use anyhow::Result;
use log::{info, warn};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

pub(super) fn verify_sha256(
    path: &Path,
    expected_sha256: Option<&str>,
    model_id: &str,
) -> Result<()> {
    let Some(expected) = expected_sha256 else {
        return Ok(());
    };
    match compute_sha256(path) {
        Ok(actual) if actual == expected => {
            info!("SHA256 verified for model {}", model_id);
            Ok(())
        }
        Ok(actual) => {
            warn!(
                "SHA256 mismatch for model {}: expected {}, got {}",
                model_id, expected, actual
            );
            let _ = fs::remove_file(path);
            Err(anyhow::anyhow!(
                "Download verification failed for model {}: file is corrupt. Please retry.",
                model_id
            ))
        }
        Err(e) => {
            let _ = fs::remove_file(path);
            Err(anyhow::anyhow!(
                "Failed to verify download for model {}: {}. Please retry.",
                model_id,
                e
            ))
        }
    }
}

/// Computes the SHA256 hex digest of a file, reading in 64KB chunks to handle large models.
pub(super) fn compute_sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
