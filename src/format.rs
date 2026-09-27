use anyhow::Result;
use regex::Regex;

use crate::notebook::{Cell, CellType, Notebook, Output};

pub fn notebook_to_llm_format(notebook: &Notebook, filename: Option<&str>, limit: Option<usize>, offset: Option<usize>) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "# Notebook: {}\n\n",
        filename.unwrap_or("notebook")
    ));

    output.push_str("## Metadata\n");
    output.push_str(&format!("- Format: nbformat {}.{}\n", notebook.nbformat, notebook.nbformat_minor));
    
    if let Some(ref lang_info) = notebook.metadata.language_info {
        output.push_str(&format!("- Language: {}\n", lang_info.name));
    }
    
    if let Some(ref kernel) = notebook.metadata.kernelspec {
        output.push_str(&format!("- Kernel: {}\n", kernel.name));
    }

    if let Some(ref warnings) = notebook.metadata.warnings {
        if !warnings.is_empty() {
            output.push_str(&format!("- Warnings: {}\n", warnings.join(", ")));
        }
    }

    output.push_str("\n---\n\n");

    for cell in &notebook.cells {
        let id_str = cell.id.as_deref().unwrap_or("<no-id>");
        output.push_str(&format!("## Cell: {} (id: {})\n\n", cell.cell_type, id_str));
        output.push_str("### Source\n");

        match &cell.cell_type {
            CellType::Code | CellType::Raw => {
                let lang = notebook.metadata.language_info.as_ref()
                    .map(|l| l.name.as_str())
                    .unwrap_or("python");
                output.push_str(&format!("```{}\n{}\n```\n", 
                    if cell.cell_type == CellType::Raw { "raw" } else { lang },
                    cell.source.as_string()
                ));
            }
            CellType::Markdown => {
                output.push_str(&cell.source.as_string());
                if !cell.source.as_string().ends_with('\n') {
                    output.push('\n');
                }
            }
        }

        if let Some(ref outputs) = cell.outputs {
            if !outputs.is_empty() {
                output.push_str("\n### Outputs\n");
                for o in outputs {
                    output.push_str(&format_output(o));
                    output.push('\n');
                }
            }
        }

        output.push_str("\n---\n\n");
    }

    apply_line_limit(output, limit, offset)
}

fn apply_line_limit(output: String, limit: Option<usize>, offset: Option<usize>) -> String {
    match (limit, offset) {
        (None, None) => output,
        _ => {
            let lines: Vec<&str> = output.lines().collect();
            let offset_idx = offset.unwrap_or(0);
            
            let lines_to_use = if let Some(lim) = limit {
                if offset_idx + lim >= lines.len() {
                    if offset_idx >= lines.len() {
                        return String::new();
                    }
                    &lines[offset_idx..]
                } else {
                    &lines[offset_idx..offset_idx + lim]
                }
            } else {
                if offset_idx >= lines.len() {
                    return String::new();
                }
                &lines[offset_idx..]
            };

            lines_to_use.join("\n") + "\n"
        }
    }
}

pub fn format_single_cell(cell: &Cell, notebook: &Notebook) -> String {
    let mut output = String::new();

    let id_str = cell.id.as_deref().unwrap_or("<no-id>");
    output.push_str(&format!("## Cell: {} (id: {})\n\n", cell.cell_type, id_str));
    output.push_str("### Source\n");

    match &cell.cell_type {
        CellType::Code | CellType::Raw => {
            let lang = notebook.metadata.language_info.as_ref()
                .map(|l| l.name.as_str())
                .unwrap_or("python");
            output.push_str(&format!("```{}\n{}\n```\n", 
                if cell.cell_type == CellType::Raw { "raw" } else { lang },
                cell.source.as_string()
            ));
        }
        CellType::Markdown => {
            output.push_str(&cell.source.as_string());
            if !cell.source.as_string().ends_with('\n') {
                output.push('\n');
            }
        }
    }

    if let Some(ref outputs) = cell.outputs {
        if !outputs.is_empty() {
            output.push_str("\n### Outputs\n");
            for o in outputs {
                output.push_str(&format_output(o));
                output.push('\n');
            }
        }
    }

    output.push_str("\n---\n");
    output
}

fn format_output(output: &Output) -> String {
    match output {
        Output::Stream { name, text } => {
            format!("- Stream ({}): {}", name, text.as_string().trim_end())
        }
        Output::ExecuteResult { data, .. } => {
            format!("- Result: {}", summarize_data(data))
        }
        Output::DisplayData { data, .. } => {
            let mime_type = data.as_object()
                .and_then(|obj| obj.keys().next())
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            format!("- Display ({}): {}", mime_type, summarize_data(data))
        }
        Output::Error { ename, evalue, traceback } => {
            let mut s = format!("- Error: {}: {}", ename, evalue);
            if !traceback.is_empty() {
                s.push_str("\n  Traceback:\n");
                for line in traceback {
                    s.push_str(&format!("  - {}\n", line));
                }
            }
            s
        }
    }
}

fn summarize_data(data: &serde_json::Value) -> String {
    if let Some(obj) = data.as_object() {
        if let Some(text) = obj.get("text/plain") {
            return truncate(&plain_text(text), 100);
        }
        if let Some(html) = obj.get("text/html") {
            return format!("<html: {} chars>", plain_text(html).len());
        }
        if obj.contains_key("image/png") || obj.contains_key("image/jpeg") {
            return "<image data>".to_string();
        }
    }
    "<data>".to_string()
}

/// nbformat allows mime values as a single string or a list of lines.
fn plain_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(lines) => lines
            .iter()
            .filter_map(|l| l.as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

pub fn llm_format_to_notebook(content: &str) -> Result<(Notebook, Vec<String>)> {
    let mut warnings = Vec::new();
    let mut notebook = Notebook::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.starts_with("## Metadata") {
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("---") {
                let meta_line = lines[i].trim();
                if meta_line.starts_with("- Language:") {
                    let lang = meta_line[11..].trim();
                    notebook.metadata.language_info = Some(crate::notebook::LanguageInfo {
                        name: lang.to_string(),
                        file_extension: None,
                        mimetype: None,
                    });
                } else if meta_line.starts_with("- Kernel:") {
                    let kernel = meta_line[9..].trim();
                    notebook.metadata.kernelspec = Some(crate::notebook::KernelSpec {
                        name: kernel.to_string(),
                        display_name: kernel.to_string(),
                    });
                }
                i += 1;
            }
        } else if line.starts_with("## Cell:") {
            let cell_result = parse_cell_header(line);
            if let Some((cell_type, cell_id)) = cell_result {
                i += 1;
                
                let (source, consumed) = parse_cell_source(&lines[i..], cell_type)?;
                i += consumed;

                let mut cell = Cell::new(cell_type, source);
                cell.id = cell_id;

                notebook.cells.push(cell);
            } else {
                warnings.push(format!("Could not parse cell header at line {}", i + 1));
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    if notebook.cells.is_empty() {
        anyhow::bail!("Could not parse notebook content. No valid cells found (minimum 1 required)");
    }

    if !warnings.is_empty() {
        notebook.metadata.warnings = Some(warnings.clone());
    }

    Ok((notebook, warnings))
}

fn parse_cell_header(line: &str) -> Option<(CellType, Option<String>)> {
    let re = Regex::new(r"^## Cell: (Code|Markdown|Raw) \(id: (.+)\)$").ok()?;
    let caps = re.captures(line)?;
    
    let cell_type = match caps.get(1)?.as_str() {
        "Code" => CellType::Code,
        "Markdown" => CellType::Markdown,
        "Raw" => CellType::Raw,
        _ => return None,
    };

    let id = caps.get(2)?.as_str().to_string();
    Some((cell_type, Some(id)))
}

fn parse_cell_source(lines: &[&str], cell_type: CellType) -> Result<(String, usize)> {
    let mut source = String::new();
    let mut consumed = 0;
    let mut in_code_block = false;
    let mut found_source = false;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if trimmed == "### Source" {
            found_source = true;
            consumed = idx + 1;
            continue;
        }

        if !found_source {
            continue;
        }

        if cell_type == CellType::Markdown {
            if trimmed.starts_with("### ") || trimmed == "---" {
                return Ok((source, consumed));
            }
            source.push_str(line);
            source.push('\n');
            consumed = idx + 1;
        } else {
            if trimmed.starts_with("```") {
                if in_code_block {
                    consumed = idx + 1;
                    return Ok((source, consumed));
                }
                in_code_block = true;
                consumed = idx + 1;
                continue;
            }

            if in_code_block {
                source.push_str(line);
                source.push('\n');
                consumed = idx + 1;
            } else if trimmed.starts_with("### ") || trimmed == "---" {
                return Ok((source, consumed));
            }
        }
    }

    Ok((source, consumed))
}
