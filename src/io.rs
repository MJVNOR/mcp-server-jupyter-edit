use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::notebook::Notebook;

pub fn read_notebook_file<P: AsRef<Path>>(path: P) -> Result<Notebook> {
    let path = path.as_ref();
    debug!("Reading notebook from: {}", path.display());

    let content = fs::read_to_string(path)
        .with_context(|| format!("Cannot read file: {}", path.display()))?;

    let notebook: Notebook = serde_json::from_str(&content)
        .with_context(|| format!("Invalid JSON format in notebook file: {}", path.display()))?;

    info!("Successfully read notebook from: {}", path.display());
    Ok(notebook)
}

pub fn write_notebook_file<P: AsRef<Path>>(path: P, notebook: &Notebook) -> Result<()> {
    let path = path.as_ref();
    debug!("Writing notebook to: {}", path.display());

    // Resolve to an absolute path so backup/temp/rename don't depend on the
    // process working directory (e.g. WSL-style paths served from a UNC cwd).
    let abs_path: PathBuf = if path.exists() {
        fs::canonicalize(path)
            .with_context(|| format!("Cannot resolve absolute path for: {}", path.display()))?
    } else if let Some(parent) = path.parent() {
        if parent.as_os_str().is_empty() {
            std::env::current_dir()
                .context("Cannot determine current directory")?
                .join(path)
        } else {
            let abs_parent = fs::canonicalize(parent).with_context(|| {
                format!("Directory does not exist: {}", parent.display())
            })?;
            match path.file_name() {
                Some(name) => abs_parent.join(name),
                None => path.to_path_buf(),
            }
        }
    } else {
        path.to_path_buf()
    };
    let path = abs_path.as_path();

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            anyhow::bail!("Directory does not exist: {}", parent.display());
        }
    }

    if path.exists() {
        backup_notebook(path)?;
    }

    let content = serde_json::to_string_pretty(notebook)
        .context("Failed to serialize notebook to JSON")?;

    // Unique temp name: concurrent writes to the same notebook must not share it.
    let temp_path =
        path.with_extension(format!("ipynb.tmp-{}", uuid::Uuid::new_v4().simple()));
    fs::write(&temp_path, content)
        .with_context(|| format!("Cannot write temporary file: {}", temp_path.display()))?;

    fs::rename(&temp_path, path)
        .with_context(|| format!("Cannot rename temp file to: {}", path.display()))?;

    info!("Successfully wrote notebook to: {}", path.display());
    Ok(())
}

pub fn backup_notebook<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let path = path.as_ref();

    if !path.exists() {
        anyhow::bail!("Cannot backup, file does not exist: {}", path.display());
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    let backup_dir = parent.join(".jupyter-edit-backups");

    if !backup_dir.exists() {
        fs::create_dir_all(&backup_dir)
            .with_context(|| format!("Cannot create backup directory: {}", backup_dir.display()))?;
    }

    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid filename: {}", path.display()))?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("{}.{timestamp}.ipynb", filename.trim_end_matches(".ipynb"));
    let backup_path = backup_dir.join(backup_filename);

    fs::copy(path, &backup_path)
        .with_context(|| format!("Cannot create backup: {}", backup_path.display()))?;

    warn!("Created backup at: {}", backup_path.display());
    Ok(backup_path)
}
