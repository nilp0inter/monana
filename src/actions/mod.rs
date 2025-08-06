use anyhow::{Context, Result};
use camino::Utf8Path;
use std::fs;
use std::os::unix::fs::symlink;

#[derive(Debug, Clone)]
pub enum Action {
    Move,
    Copy,
    Symlink,
    Hardlink,
    Custom(String),
}

impl Action {
    pub fn execute(&self, source: &Utf8Path, destination: &Utf8Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {parent}"))?;
        }

        match self {
            Action::Move => fs::rename(source, destination)
                .with_context(|| format!("Failed to move {source} to {destination}")),
            Action::Copy => fs::copy(source, destination)
                .map(|_| ())
                .with_context(|| format!("Failed to copy {source} to {destination}")),
            Action::Symlink => symlink(source, destination)
                .with_context(|| format!("Failed to symlink {source} to {destination}")),
            Action::Hardlink => fs::hard_link(source, destination)
                .with_context(|| format!("Failed to hardlink {source} to {destination}")),
            Action::Custom(command) => execute_custom_command(command, source, destination),
        }
    }
}

fn execute_custom_command(command: &str, source: &Utf8Path, destination: &Utf8Path) -> Result<()> {
    let cmd = command
        .replace("{source}", source.as_str())
        .replace("{destination}", destination.as_str());

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .with_context(|| format!("Failed to execute custom command: {cmd}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Custom command failed: {}", stderr);
    }

    Ok(())
}
