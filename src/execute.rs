use anyhow::{anyhow, Context, Result};
use std::process::Command;

use crate::io::{read_notebook_file, write_notebook_file};
use crate::notebook::{CellType, Notebook};

pub struct ExecuteNotebookRequest {
    pub path: String,
    pub kernel_name: Option<String>,
    pub allow_errors: bool,
}

pub struct ExecuteCellRequest {
    pub path: String,
    pub cell_id: String,
    pub kernel_name: Option<String>,
}

fn nbconvert_base_args(path: &str, kernel_name: &Option<String>, allow_errors: bool) -> Vec<String> {
    let mut args = vec![
        "nbconvert".to_string(),
        "--to".to_string(),
        "notebook".to_string(),
        "--execute".to_string(),
        "--inplace".to_string(),
    ];
    if let Some(k) = kernel_name {
        if !k.trim().is_empty() {
            args.push(format!("--ExecutePreprocessor.kernel_name={k}"));
        }
    }
    if allow_errors {
        args.push("--allow-errors".to_string());
    }
    args.push(path.to_string());
    args
}

fn run_jupyter(args: &[String]) -> Result<std::process::Output> {
    Command::new("jupyter")
        .args(args)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow!("'jupyter' not found in PATH. Install Jupyter (pip install notebook nbconvert ipykernel) to use execution tools")
            } else {
                anyhow!("Failed to spawn 'jupyter': {e}")
            }
        })
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    } else {
        s.to_string()
    }
}

pub fn execute_notebook(req: ExecuteNotebookRequest) -> Result<String> {
    // Validate it's a readable notebook before shelling out, and normalize
    // it (code cells require `outputs` to be a list) so nbconvert accepts it.
    let mut notebook = read_notebook_file(&req.path)?;
    let total = notebook.cells.len();
    write_notebook_file(&req.path, &mut notebook)?;

    let args = nbconvert_base_args(&req.path, &req.kernel_name, req.allow_errors);
    let out = run_jupyter(&args)?;

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(anyhow!(
            "nbconvert failed ({} cells): {}\n{}",
            total,
            truncate_chars(stderr.trim(), 2000),
            truncate_chars(stdout.trim(), 500)
        ));
    }

    // Re-read to surface per-cell errors without dumping full outputs.
    let executed = read_notebook_file(&req.path)?;
    let errors: Vec<String> = executed
        .cells
        .iter()
        .filter_map(|c| {
            c.outputs.as_ref().and_then(|o| {
                o.iter().find_map(|out| match out {
                    crate::notebook::Output::Error { ename, evalue, .. } => Some(format!(
                        "{}: {ename}: {evalue}",
                        c.id.as_deref().unwrap_or("<no-id>")
                    )),
                    _ => None,
                })
            })
        })
        .collect();

    let mut msg = format!("Executed {total} cells in-place: {}", req.path);
    if !errors.is_empty() {
        msg.push_str(&format!("\n{} cell(s) with errors:\n- {}", errors.len(), errors.join("\n- ")));
    }
    let tail = format!("{stdout}\n{stderr}");
    if !tail.trim().is_empty() {
        msg.push_str(&format!("\nlog: {}", truncate_chars(tail.trim(), 1000)));
    }
    Ok(msg)
}

pub fn execute_cell(req: ExecuteCellRequest) -> Result<String> {
    let mut notebook = read_notebook_file(&req.path)?;

    let idx = notebook
        .cells
        .iter()
        .position(|c| c.id.as_ref().map(|id| id.as_str()) == Some(req.cell_id.as_str()))
        .ok_or_else(|| anyhow!("Cell not found with ID: {}", req.cell_id))?;

    if notebook.cells[idx].cell_type != CellType::Code {
        anyhow::bail!("Only code cells can be executed (cell is {})", notebook.cells[idx].cell_type);
    }

    // ponytail: stateless single-cell run (no kernel memory of sibling cells); use execute_notebook or a persistent kernel if state matters.
    let mut single = Notebook {
        nbformat: notebook.nbformat,
        nbformat_minor: notebook.nbformat_minor,
        metadata: notebook.metadata.clone(),
        cells: vec![notebook.cells[idx].clone()],
    };

    let tmp = tempfile::Builder::new()
        .suffix(".ipynb")
        .tempfile()
        .context("Cannot create temp notebook file")?;
    single.normalize_outputs();
    serde_json::to_writer_pretty(tmp.as_file(), &single).context("Cannot write temp notebook")?;
    let tmp_path = tmp.path().to_string_lossy().to_string();

    let args = nbconvert_base_args(&tmp_path, &req.kernel_name, false);
    let out = run_jupyter(&args)?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("nbconvert failed for cell {}: {}", req.cell_id, truncate_chars(stderr.trim(), 2000)));
    }

    let executed: Notebook = {
        let data = std::fs::read(tmp.path()).context("Cannot read executed temp notebook")?;
        serde_json::from_slice(&data).context("Cannot parse executed temp notebook")?
    };
    let exec_cell = executed.cells.into_iter().next().ok_or_else(|| anyhow!("Executed notebook has no cells"))?;

    notebook.cells[idx].outputs = exec_cell.outputs;
    notebook.cells[idx].execution_count = exec_cell.execution_count;
    write_notebook_file(&req.path, &mut notebook)?;

    Ok(format!("Executed cell {} in-place: {}", req.cell_id, req.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_full_defaults() {
        let args = nbconvert_base_args("a.ipynb", &None, false);
        assert_eq!(args, vec!["nbconvert", "--to", "notebook", "--execute", "--inplace", "a.ipynb"]);
    }

    #[test]
    fn args_kernel_and_allow_errors() {
        let args = nbconvert_base_args("a.ipynb", &Some("python3".into()), true);
        assert!(args.contains(&"--ExecutePreprocessor.kernel_name=python3".to_string()));
        assert!(args.contains(&"--allow-errors".to_string()));
        assert_eq!(args.last().unwrap(), "a.ipynb");
    }

    #[test]
    fn truncate_keeps_char_boundary() {
        assert_eq!(truncate_chars("tamaño ✅ datos", 6), "tamaño...");
        assert_eq!(truncate_chars("abc", 100), "abc");
    }

    #[test]
    fn execute_missing_cell_errors_without_spawning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nb.ipynb");
        let nb = Notebook::new();
        std::fs::write(&path, serde_json::to_string(&nb).unwrap()).unwrap();
        let err = execute_cell(ExecuteCellRequest {
            path: path.to_string_lossy().to_string(),
            cell_id: "nope".into(),
            kernel_name: None,
        })
        .unwrap_err();
        assert!(err.to_string().contains("Cell not found"));
    }
}
