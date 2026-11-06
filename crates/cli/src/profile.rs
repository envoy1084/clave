//! Profile paths and permissions (`architecture/cli.md`, Profiles and files).

use std::{
    fs,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use crate::error::{Error, Result};

/// `TECPAN_HOME`, else `~/.tecpan`.
pub(crate) fn home() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("TECPAN_HOME").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".tecpan"))
        .ok_or_else(|| Error::failed("HOME is not set; set TECPAN_HOME"))
}

/// Profile names match `^[a-z0-9][a-z0-9_-]{0,31}$`.
pub(crate) fn validate_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let rest_ok =
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if first_ok && rest_ok && name.len() <= 32 {
        Ok(())
    } else {
        Err(Error::usage(format!(
            "invalid profile name \"{name}\": use 1-32 of a-z, 0-9, _ and -, starting with a letter or digit"
        )))
    }
}

/// One member profile on this machine.
#[derive(Clone, Debug)]
pub(crate) struct Profile {
    pub(crate) name: String,
    pub(crate) dir: PathBuf,
}

impl Profile {
    pub(crate) fn resolve(home: &Path, name: &str) -> Result<Self> {
        validate_name(name)?;
        Ok(Self {
            name: name.to_owned(),
            dir: home.join("profiles").join(name),
        })
    }

    /// The socket path the daemon wrote to `daemon.addr`, else `<dir>/daemon.sock`.
    pub(crate) fn socket(&self) -> PathBuf {
        fs::read_to_string(self.dir.join("daemon.addr"))
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .map_or_else(|| self.dir.join("daemon.sock"), PathBuf::from)
    }

    pub(crate) fn token(&self) -> PathBuf {
        self.dir.join("daemon.token")
    }

    pub(crate) fn pid_file(&self) -> PathBuf {
        self.dir.join("daemon.pid")
    }

    pub(crate) fn daemon_log(&self) -> PathBuf {
        self.dir.join("daemon.log")
    }

    pub(crate) fn keeper_log(&self) -> PathBuf {
        self.dir.join("keeper.log")
    }

    /// Marker that Touch ID is enabled: checking it never prompts, unlike probing the keychain item.
    pub(crate) fn touch_id_marker(&self) -> PathBuf {
        self.dir.join("touch-id")
    }

    pub(crate) fn exists(&self) -> bool {
        self.dir.join("profile.json").exists()
    }

    pub(crate) fn create_dir(&self) -> Result<()> {
        create_private_dir(&self.dir)
    }

    /// The pid recorded by the daemon, if any.
    pub(crate) fn pid(&self) -> Option<u32> {
        fs::read_to_string(self.pid_file())
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// The `-P` flag to repeat in hints (empty for `default`).
    pub(crate) fn flag(&self) -> String {
        if self.name == "default" {
            String::new()
        } else {
            format!("-P {} ", self.name)
        }
    }
}

/// Every profile directory under `home`.
pub(crate) fn list(home: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(home.join("profiles"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| validate_name(n).is_ok())
        .collect();
    names.sort();
    names
}

/// Creates `dir` and its parents; the leaf is `0700`.
pub(crate) fn create_private_dir(dir: &Path) -> Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

/// Writes `bytes` to a new or truncated `0600` file.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    Ok(())
}

/// Permission bits beyond the owner's (`mode & 0o077`), or `None` when the path is missing.
pub(crate) fn group_or_other_bits(path: &Path) -> Option<u32> {
    fs::metadata(path)
        .ok()
        .map(|m| m.permissions().mode() & 0o077)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_names() {
        for ok in ["default", "ana", "a", "bea-2", "c_1", "0x"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", "Ana", "-a", "_a", "a b", "a/b", &"a".repeat(33)] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn private_dir_and_file_modes() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profiles/ana");
        create_private_dir(&dir).unwrap();
        assert_eq!(group_or_other_bits(&dir), Some(0));
        let file = dir.join("x");
        write_private(&file, b"1").unwrap();
        assert_eq!(group_or_other_bits(&file), Some(0));
        assert_eq!(list(tmp.path()), vec!["ana".to_owned()]);
    }
}
