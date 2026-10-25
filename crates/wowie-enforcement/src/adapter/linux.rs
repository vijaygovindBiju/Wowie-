use super::EnforcementAdapter;
use crate::error::EnforcementError;
use crate::model::EnforcementTarget;
use async_trait::async_trait;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const WOWIE_SECTION_START: &str = "# --- BEGIN WOWIE TEMPORARY DISTRACTION BLOCKLIST ---";
pub const WOWIE_SECTION_END: &str = "# --- END WOWIE TEMPORARY DISTRACTION BLOCKLIST ---";

#[derive(Debug)]
pub struct LinuxEnforcementAdapter {
    blocklist_path: PathBuf,
    backup_path: PathBuf,
    is_shared_system_file: bool,
    active_targets: Arc<Mutex<HashSet<EnforcementTarget>>>,
}

impl Default for LinuxEnforcementAdapter {
    fn default() -> Self {
        Self::new(std::env::temp_dir().join("wowie_enforced_blocks.txt"))
    }
}

impl LinuxEnforcementAdapter {
    pub fn new(blocklist_path: PathBuf) -> Self {
        let backup_path = blocklist_path.with_extension(format!(
            "{}.wowie_backup",
            blocklist_path
                .extension()
                .map_or("bak", |ext| ext.to_str().unwrap_or("bak"))
        ));

        // Determine if this is a shared file (e.g. /etc/hosts or an existing user file)
        let path_str = blocklist_path.to_string_lossy().to_lowercase();
        let is_system_path = path_str.starts_with("/etc/")
            || blocklist_path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| {
                    name.eq_ignore_ascii_case("hosts") || name.contains("etc_hosts")
                });

        let is_shared_system_file = if is_system_path {
            true
        } else if blocklist_path.exists() {
            if let Ok(content) = fs::read_to_string(&blocklist_path) {
                content.lines().any(|l| {
                    let t = l.trim();
                    !t.is_empty()
                        && !t.starts_with("# Wowie")
                        && !t.starts_with("# Auto-generated")
                        && !t.contains("WOWIE TEMPORARY")
                })
            } else {
                false
            }
        } else {
            false
        };

        let adapter = Self {
            blocklist_path,
            backup_path,
            is_shared_system_file,
            active_targets: Arc::new(Mutex::new(HashSet::new())),
        };

        // Crash recovery:
        // If shared system file: strip only any stale Wowie block section; NEVER delete the file!
        // If dedicated scratch file: remove it cleanly.
        if adapter.blocklist_path.exists() {
            if adapter.is_shared_system_file {
                let _ = adapter.strip_wowie_section();
            } else {
                let _ = fs::remove_file(&adapter.blocklist_path);
            }
        }

        adapter
    }

    pub fn blocklist_path(&self) -> &Path {
        &self.blocklist_path
    }

    pub fn backup_path(&self) -> &Path {
        &self.backup_path
    }

    pub fn is_shared_system_file(&self) -> bool {
        self.is_shared_system_file
    }

    /// Atomically write file contents using a temporary sibling file and rename,
    /// providing actionable errors on permission denial.
    fn atomic_write_file(path: &Path, content: &str) -> Result<(), EnforcementError> {
        let temp_path = path.with_extension(format!("wowie_tmp_{}", uuid::Uuid::new_v4()));
        if let Err(e) = fs::write(&temp_path, content) {
            let details = if e.kind() == std::io::ErrorKind::PermissionDenied {
                format!(
                    "Permission denied writing {}. Modifying system hosts requires root/elevated privileges.",
                    path.display()
                )
            } else {
                format!("Failed to write {}: {e}", path.display())
            };
            return Err(EnforcementError::AdapterFailure {
                adapter: "linux",
                details,
            });
        }

        if let Err(_e) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            if let Err(e2) = fs::write(path, content) {
                let details = if e2.kind() == std::io::ErrorKind::PermissionDenied {
                    format!(
                        "Permission denied writing {}. Modifying system hosts requires root/elevated privileges.",
                        path.display()
                    )
                } else {
                    format!("Failed to update {}: {e2}", path.display())
                };
                return Err(EnforcementError::AdapterFailure {
                    adapter: "linux",
                    details,
                });
            }
        }

        Ok(())
    }

    /// Safely strip any demarcated Wowie block section from the target file,
    /// preserving all user and system configuration lines.
    fn strip_wowie_section(&self) -> Result<(), EnforcementError> {
        if !self.blocklist_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.blocklist_path).map_err(|e| {
            EnforcementError::AdapterFailure {
                adapter: "linux",
                details: format!("Failed to read {}: {e}", self.blocklist_path.display()),
            }
        })?;

        let cleaned = Self::filter_out_wowie_section(&content);
        if cleaned != content {
            Self::atomic_write_file(&self.blocklist_path, &cleaned)?;
        }

        Ok(())
    }

    fn filter_out_wowie_section(content: &str) -> String {
        let mut in_wowie_section = false;
        let mut preserved = Vec::new();

        for line in content.lines() {
            if line.contains(WOWIE_SECTION_START) {
                in_wowie_section = true;
                continue;
            }
            if line.contains(WOWIE_SECTION_END) {
                in_wowie_section = false;
                continue;
            }
            if !in_wowie_section {
                preserved.push(line);
            }
        }

        let mut result = preserved.join("\n");
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result
    }

    /// User recovery: restores the target file from the created backup.
    pub fn restore_from_backup(&self) -> Result<(), EnforcementError> {
        if self.backup_path.exists() {
            fs::copy(&self.backup_path, &self.blocklist_path).map_err(|e| {
                EnforcementError::AdapterFailure {
                    adapter: "linux",
                    details: format!(
                        "Failed to restore {} from {}: {e}",
                        self.blocklist_path.display(),
                        self.backup_path.display()
                    ),
                }
            })?;
        } else {
            // If no backup file, at least strip any Wowie section
            self.strip_wowie_section()?;
        }
        Ok(())
    }

    fn sync_blocklist_file(
        &self,
        targets: &HashSet<EnforcementTarget>,
    ) -> Result<(), EnforcementError> {
        if self.is_shared_system_file {
            // 1. Ensure backup exists before any mutation
            if self.blocklist_path.exists() && !self.backup_path.exists() {
                let _ = fs::copy(&self.blocklist_path, &self.backup_path);
            }

            let existing_content = if self.blocklist_path.exists() {
                fs::read_to_string(&self.blocklist_path).map_err(|e| {
                    EnforcementError::AdapterFailure {
                        adapter: "linux",
                        details: format!("Failed to read {}: {e}", self.blocklist_path.display()),
                    }
                })?
            } else {
                String::new()
            };

            let base_content = Self::filter_out_wowie_section(&existing_content);

            if targets.is_empty() {
                Self::atomic_write_file(&self.blocklist_path, &base_content)?;
                return Ok(());
            }

            let mut out = base_content;
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }

            let _ = writeln!(out, "{WOWIE_SECTION_START}");
            out.push_str("# Auto-generated and automatically cleared on session end\n");
            for target in targets {
                let _ = writeln!(out, "127.0.0.1 {}", target.raw_value());
                let _ = writeln!(out, "::1 {}", target.raw_value());
            }
            let _ = writeln!(out, "{WOWIE_SECTION_END}");

            Self::atomic_write_file(&self.blocklist_path, &out)
        } else {
            // Dedicated standalone blocklist file
            if targets.is_empty() {
                if self.blocklist_path.exists() {
                    let _ = fs::remove_file(&self.blocklist_path);
                }
                return Ok(());
            }

            let mut lines = Vec::new();
            lines.push("# Wowie Temporary Distraction Enforcement Blocklist".to_string());
            lines.push("# Auto-generated and automatically cleared on session end".to_string());
            for target in targets {
                lines.push(format!("127.0.0.1 {}", target.raw_value()));
                lines.push(format!("::1 {}", target.raw_value()));
            }

            Self::atomic_write_file(&self.blocklist_path, &lines.join("\n"))
        }
    }
}

#[async_trait]
impl EnforcementAdapter for LinuxEnforcementAdapter {
    fn name(&self) -> &'static str {
        "linux_desktop"
    }

    async fn apply_block(&self, target: &EnforcementTarget) -> Result<(), EnforcementError> {
        {
            let mut targets = self.active_targets.lock().unwrap();
            targets.insert(target.clone());
            self.sync_blocklist_file(&targets)?;
        }

        // Send a notification letting the user know the target is temporarily blocked
        let _ = tokio::process::Command::new("notify-send")
            .args([
                "-a",
                "Wowie",
                "Distraction Block Activated",
                &format!(
                    "{} is now blocked for the remainder of this commitment.",
                    target.raw_value()
                ),
            ])
            .output()
            .await;

        Ok(())
    }

    async fn remove_block(&self, target: &EnforcementTarget) -> Result<(), EnforcementError> {
        let mut targets = self.active_targets.lock().unwrap();
        targets.remove(target);
        self.sync_blocklist_file(&targets)
    }

    async fn cleanup_all(&self) -> Result<(), EnforcementError> {
        let mut targets = self.active_targets.lock().unwrap();
        targets.clear();
        self.sync_blocklist_file(&targets)
    }
}

impl Drop for LinuxEnforcementAdapter {
    fn drop(&mut self) {
        if self.is_shared_system_file {
            // If shared file, strip only the Wowie demarcated section! NEVER delete the file!
            let _ = self.strip_wowie_section();
        } else if self.blocklist_path.exists() {
            // Dedicated temporary file: safe to remove
            let _ = fs::remove_file(&self.blocklist_path);
        }
    }
}
