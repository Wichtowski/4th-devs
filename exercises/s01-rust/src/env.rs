use anyhow::{Context, Result};
use dotenvy::from_filename_override;
use std::path::PathBuf;

pub fn load_shared_env() -> Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("..").join(".env"),
        manifest_dir.join(".env"),
    ];
    for path in candidates {
        if path.exists() {
            from_filename_override(&path)
                .with_context(|| format!("Failed to load {}", path.display()))?;
            break;
        }
    }
    Ok(())
}
