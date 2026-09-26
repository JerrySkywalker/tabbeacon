//! Exact-owned project Hook declarations for the admitted Cursor CLI contract.
//! The caller selects a workspace explicitly; no ambient daily profile is edited.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use serde_json::{Map, Value, json};

const EVENTS: [&str; 4] = ["sessionStart", "beforeSubmitPrompt", "stop", "sessionEnd"];
const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const OWNED_MARKER: &str = "__cursor-hook-v1";

/// The inspected state of one explicit project Hook file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorHookState {
    NotInstalled,
    Partial,
    Installed,
    Drift,
}

impl CursorHookState {
    /// Stable machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::Partial => "partial",
            Self::Installed => "installed",
            Self::Drift => "drift",
        }
    }
}

/// Scope for one project-owned Hook declaration set.
#[derive(Debug, Clone)]
pub struct CursorHookIntegration {
    workspace: PathBuf,
    executable: PathBuf,
}

impl CursorHookIntegration {
    /// Binds operations to an existing explicit workspace and absolute executable.
    ///
    /// # Errors
    ///
    /// Rejects absent, linked or non-absolute boundaries.
    pub fn new(workspace: &Path, executable: &Path) -> io::Result<Self> {
        if !workspace.is_absolute() || !executable.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "absolute paths required",
            ));
        }
        reject_link(workspace)?;
        if !workspace.is_dir() || !executable.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "workspace or executable missing",
            ));
        }
        reject_link(executable)?;
        Ok(Self {
            workspace: workspace.canonicalize()?,
            // Keep the launchable Windows spelling. `canonicalize` can add a
            // verbatim `\\?\` prefix that Cursor's command shell cannot use.
            executable: executable.to_path_buf(),
        })
    }

    fn directory(&self) -> PathBuf {
        self.workspace.join(".cursor")
    }

    fn path(&self) -> PathBuf {
        self.directory().join("hooks.json")
    }

    fn command(&self) -> String {
        format!("\"{}\" {OWNED_MARKER}", self.executable.display())
    }

    fn read(&self) -> io::Result<(Option<Vec<u8>>, Value)> {
        let directory = self.directory();
        let path = self.path();
        reject_link(&directory)?;
        reject_link(&path)?;
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok((None, json!({"version":1,"hooks":{}})));
            }
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take((MAX_CONFIG_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Cursor Hook config too large",
            ));
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid Cursor Hook config")
        })?;
        if value.get("version").and_then(Value::as_u64) != Some(1)
            || !value.get("hooks").is_some_and(Value::is_object)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported Cursor Hook config",
            ));
        }
        Ok((Some(bytes), value))
    }

    fn event_entries<'a>(hooks: &'a Map<String, Value>, event: &str) -> io::Result<&'a [Value]> {
        match hooks.get(event) {
            None => Ok(&[]),
            Some(Value::Array(entries)) => Ok(entries),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Cursor Hook entries",
            )),
        }
    }

    fn state_for(&self, value: &Value) -> io::Result<CursorHookState> {
        let hooks = value["hooks"]
            .as_object()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid Cursor Hook map"))?;
        let expected = self.command();
        let mut owned = 0;
        for (event, entries) in hooks {
            if !EVENTS.contains(&event.as_str())
                && entries.as_array().is_some_and(|entries| {
                    entries.iter().any(|entry| {
                        entry
                            .get("command")
                            .and_then(Value::as_str)
                            .is_some_and(|command| command.contains(OWNED_MARKER))
                    })
                })
            {
                return Ok(CursorHookState::Drift);
            }
        }
        for event in EVENTS {
            let entries = Self::event_entries(hooks, event)?;
            let mut matches = 0;
            for entry in entries {
                let Some(command) = entry.get("command").and_then(Value::as_str) else {
                    continue;
                };
                if command == expected {
                    if entry.get("timeout").and_then(Value::as_u64) != Some(3)
                        || entry.get("failClosed").and_then(Value::as_bool) != Some(false)
                        || entry.as_object().is_none_or(|fields| fields.len() != 3)
                    {
                        return Ok(CursorHookState::Drift);
                    }
                    matches += 1;
                } else if command.contains(OWNED_MARKER) {
                    return Ok(CursorHookState::Drift);
                }
            }
            if matches > 1 {
                return Ok(CursorHookState::Drift);
            }
            owned += matches;
        }
        Ok(match owned {
            0 => CursorHookState::NotInstalled,
            4 => CursorHookState::Installed,
            _ => CursorHookState::Partial,
        })
    }

    /// Inspects only the selected project Hook file.
    ///
    /// # Errors
    ///
    /// Returns errors for unreadable or unsupported configurations.
    pub fn check(&self) -> io::Result<CursorHookState> {
        let (_, value) = self.read()?;
        self.state_for(&value)
    }

    /// Reconciles only missing exact-owned entries under a project-local lock.
    ///
    /// # Errors
    ///
    /// Refuses modified ownership, invalid config, linked paths and concurrent drift.
    pub fn install(&self) -> io::Result<CursorHookState> {
        self.change(true)
    }

    /// Removes only exact-owned entries and retains all foreign content.
    ///
    /// # Errors
    ///
    /// Refuses modified ownership, invalid config, linked paths and concurrent drift.
    pub fn uninstall(&self) -> io::Result<CursorHookState> {
        self.change(false)
    }

    fn change(&self, install: bool) -> io::Result<CursorHookState> {
        if !install && self.check()? == CursorHookState::NotInstalled {
            return Ok(CursorHookState::NotInstalled);
        }
        let directory = self.directory();
        reject_link(&directory)?;
        fs::create_dir_all(&directory)?;
        let lock_path = directory.join("tabbeacon-hook.lock");
        reject_link(&lock_path)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.lock()?;
        let result = self.change_locked(install);
        File::unlock(&lock)?;
        result
    }

    fn change_locked(&self, install: bool) -> io::Result<CursorHookState> {
        let (before, mut value) = self.read()?;
        let state = self.state_for(&value)?;
        if state == CursorHookState::Drift {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Cursor Hook ownership drift",
            ));
        }
        if (install && state == CursorHookState::Installed)
            || (!install && state == CursorHookState::NotInstalled)
        {
            return Ok(state);
        }
        let command = self.command();
        let hooks = value["hooks"]
            .as_object_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid Cursor Hook map"))?;
        for event in EVENTS {
            let entries = hooks.entry(event).or_insert_with(|| json!([]));
            let array = entries.as_array_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid Cursor Hook entries")
            })?;
            if install {
                if !array.iter().any(|entry| entry["command"] == command) {
                    array.push(json!({"command":command,"timeout":3,"failClosed":false}));
                }
            } else {
                array.retain(|entry| entry["command"] != command);
            }
        }
        let bytes = serde_json::to_vec_pretty(&value).map_err(io::Error::other)?;
        self.commit_if_unchanged(before.as_deref(), &bytes)?;
        self.check()
    }

    fn commit_if_unchanged(&self, before: Option<&[u8]>, replacement: &[u8]) -> io::Result<()> {
        let path = self.path();
        reject_link(&path)?;
        if let Some(expected) = before {
            // The target-file lock joins our project lock. It blocks ordinary
            // in-place foreign writes while the final comparison and atomic
            // replacement occur, matching the Codex integration's safety
            // boundary. A foreign atomic replacement remains an external race.
            let mut target = OpenOptions::new().read(true).write(true).open(&path)?;
            target.lock()?;
            let result = (|| {
                let mut actual = Vec::new();
                std::io::Read::by_ref(&mut target)
                    .take((MAX_CONFIG_BYTES + 1) as u64)
                    .read_to_end(&mut actual)?;
                if actual != expected {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "Cursor Hook config changed",
                    ));
                }
                let mut file = AtomicWriteFile::options().open(&path)?;
                file.write_all(replacement)?;
                file.flush()?;
                file.commit()
            })();
            File::unlock(&target)?;
            result
        } else {
            // Link a complete same-directory file into a previously absent
            // destination. `hard_link` refuses an intervening foreign create;
            // an atomic replace of a newly created foreign Hook is forbidden.
            let staging = self.directory().join(format!(
                "tabbeacon-hook-{}-{}.tmp",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(io::Error::other)?
                    .as_nanos()
            ));
            let mut temporary = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&staging)?;
            let result = (|| {
                temporary.write_all(replacement)?;
                temporary.sync_all()?;
                fs::hard_link(&staging, &path)
            })();
            drop(temporary);
            let cleanup = fs::remove_file(&staging);
            result?;
            cleanup
        }
    }
}

fn reject_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "linked Cursor Hook path refused",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_entries_preserve_foreign_hooks_and_reconcile_idempotently() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("tabbeacon.exe");
        fs::write(&exe, b"synthetic executable").unwrap();
        let directory = root.path().join(".cursor");
        fs::create_dir(&directory).unwrap();
        let path = directory.join("hooks.json");
        fs::write(&path, br#"{"version":1,"foreign":{"key":"untouched"},"hooks":{"stop":[{"command":"foreign"}]}}"#).unwrap();
        let integration = CursorHookIntegration::new(root.path(), &exe).unwrap();
        assert_eq!(integration.check().unwrap(), CursorHookState::NotInstalled);
        assert_eq!(integration.install().unwrap(), CursorHookState::Installed);
        let installed = fs::read(&path).unwrap();
        assert_eq!(integration.install().unwrap(), CursorHookState::Installed);
        assert_eq!(fs::read(&path).unwrap(), installed);
        let value: Value = serde_json::from_slice(&installed).unwrap();
        assert_eq!(value["hooks"]["stop"][0]["command"], "foreign");
        assert_eq!(value["foreign"]["key"], "untouched");
        assert_eq!(
            integration.uninstall().unwrap(),
            CursorHookState::NotInstalled
        );
        let uninstalled = fs::read(&path).unwrap();
        assert_eq!(
            integration.uninstall().unwrap(),
            CursorHookState::NotInstalled
        );
        assert_eq!(fs::read(&path).unwrap(), uninstalled);
        let value: Value = serde_json::from_slice(&uninstalled).unwrap();
        assert_eq!(value["hooks"]["stop"][0]["command"], "foreign");
    }

    #[test]
    fn modified_owned_entry_refuses_writes() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("tabbeacon.exe");
        fs::write(&exe, b"synthetic executable").unwrap();
        let integration = CursorHookIntegration::new(root.path(), &exe).unwrap();
        integration.install().unwrap();
        let path = root.path().join(".cursor/hooks.json");
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["hooks"]["stop"][0]["timeout"] = json!(10);
        let changed = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &changed).unwrap();
        assert_eq!(integration.check().unwrap(), CursorHookState::Drift);
        assert!(integration.uninstall().is_err());
        assert_eq!(fs::read(&path).unwrap(), changed);
    }

    #[test]
    fn uninstall_without_ownership_does_not_create_project_configuration() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("tabbeacon.exe");
        fs::write(&exe, b"synthetic executable").unwrap();
        let integration = CursorHookIntegration::new(root.path(), &exe).unwrap();
        assert_eq!(
            integration.uninstall().unwrap(),
            CursorHookState::NotInstalled
        );
        assert!(!root.path().join(".cursor").exists());
    }

    #[test]
    fn partial_reconcile_adds_only_missing_owned_entries() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("tabbeacon.exe");
        fs::write(&exe, b"synthetic executable").unwrap();
        let integration = CursorHookIntegration::new(root.path(), &exe).unwrap();
        integration.install().unwrap();
        let path = root.path().join(".cursor/hooks.json");
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["hooks"]["sessionEnd"] = json!([]);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(integration.check().unwrap(), CursorHookState::Partial);
        assert_eq!(integration.install().unwrap(), CursorHookState::Installed);
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["hooks"]["sessionStart"].as_array().unwrap().len(), 1);
        assert_eq!(value["hooks"]["sessionEnd"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn commit_refuses_an_external_change_after_initial_read() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("tabbeacon.exe");
        fs::write(&exe, b"synthetic executable").unwrap();
        let integration = CursorHookIntegration::new(root.path(), &exe).unwrap();
        integration.install().unwrap();
        let path = integration.path();
        let expected = fs::read(&path).unwrap();
        let foreign = br#"{"version":1,"hooks":{"stop":[{"command":"foreign"}]}}"#;
        fs::write(&path, foreign).unwrap();
        assert_eq!(
            integration
                .commit_if_unchanged(Some(&expected), b"replacement")
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(fs::read(&path).unwrap(), foreign);
    }

    #[test]
    fn absent_target_commit_never_replaces_an_intervening_foreign_create() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("tabbeacon.exe");
        fs::write(&exe, b"synthetic executable").unwrap();
        let integration = CursorHookIntegration::new(root.path(), &exe).unwrap();
        fs::create_dir(integration.directory()).unwrap();
        let foreign = br#"{"version":1,"hooks":{"stop":[{"command":"foreign"}]}}"#;
        fs::write(integration.path(), foreign).unwrap();
        assert!(
            integration
                .commit_if_unchanged(None, b"replacement")
                .is_err()
        );
        assert_eq!(fs::read(integration.path()).unwrap(), foreign);
    }
}
