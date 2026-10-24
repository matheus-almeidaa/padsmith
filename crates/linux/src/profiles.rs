use anyhow::{Context, Result};
use padsmith_core::Profile;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn load(directory: &Path) -> Result<BTreeMap<String, Profile>> {
    fs::create_dir_all(directory)?;
    let mut profiles = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "toml") {
            let profile: Profile = toml::from_str(&fs::read_to_string(&path)?)
                .with_context(|| format!("Perfil inválido: {}", path.display()))?;
            crate::validate_profile(&profile)?;
            anyhow::ensure!(
                profiles.insert(profile.id.clone(), profile).is_none(),
                "IDs de perfil duplicados"
            );
        }
    }
    if profiles.is_empty() {
        let profile = Profile::default();
        save(directory, &profile)?;
        profiles.insert(profile.id.clone(), profile);
    }
    Ok(profiles)
}
pub fn save(directory: &Path, profile: &Profile) -> Result<()> {
    crate::validate_profile(profile)?;
    atomic_write(
        &directory.join(format!("{}.toml", profile.id)),
        toml::to_string_pretty(profile)?.as_bytes(),
    )
}
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temp = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
    // Only remove a temporary file that this invocation actually created.
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    let result = (|| -> Result<()> {
        file.write_all(data)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
