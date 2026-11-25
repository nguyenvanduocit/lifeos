//! The app's private files: a directory only the owner's OS user can enter, files only it can read.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) fn read<T: DeserializeOwned>(dir: &Path, name: &str) -> Result<Option<T>> {
    let path = dir.join(name);
    let metadata = match path.symlink_metadata() {
        Ok(m) => m,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    ensure!(metadata.is_file() && metadata.len() <= 64 * 1024, "{name} is not a Google file lifeos wrote");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(metadata.permissions().mode() & 0o077 == 0, "{} must be private", path.display());
    }
    let text = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&text).with_context(|| format!("{} is unreadable", path.display()))?))
}

pub(crate) fn write(dir: &Path, name: &str, value: &impl Serialize) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)?;
    // A temporary file is made readable by its owner only.
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(serde_json::to_string(value)?.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(dir.join(name)).map_err(|err| err.error)?;
    Ok(())
}

pub(crate) fn remove(dir: &Path, name: &str) -> Result<()> {
    match fs::remove_file(dir.join(name)) {
        Err(err) if err.kind() != ErrorKind::NotFound => Err(err.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn files_are_private() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().join("google");
        super::write(&dir, "x.json", &serde_json::json!({"a": 1})).unwrap();
        let back: serde_json::Value = super::read(&dir, "x.json").unwrap().unwrap();
        assert_eq!(back["a"], 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
            assert_eq!(std::fs::metadata(dir.join("x.json")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        super::remove(&dir, "x.json").unwrap();
        super::remove(&dir, "x.json").unwrap();
        assert!(super::read::<serde_json::Value>(&dir, "x.json").unwrap().is_none());
    }
}
