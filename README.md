# MCP Jupyter Edit Server

An MCP server that provides tools for reading and editing Jupyter notebooks (`.ipynb` files) in an LLM-friendly format.

This server is meant for working with integrated VSCode Jupyter or dropping in the environment where there is no Jupyter Lab installed. If you prefer using Jupyter Lab consider using this MCP as it has more capabilities: <https://github.com/datalayer/jupyter-mcp-server>

Ofc I've completely vibecoded it.

## Features

- Read entire notebooks in LLM-friendly markdown format
- Write notebooks from LLM-friendly format
- List cells with IDs, types, and positions
- Get individual cell content
- Add, update, and delete cells by ID

## Installation

Clone the repo

CD to the repo

```bash
cargo build --release
```

Retrieve the binary from `./target/release`

## Usage

### OpenCode Configuration

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "jupyter-edit": {
      "type": "local",
      "command": ["/path/to/mcp-server-jupyter-edit"],
      "enabled": true,
      "timeout": 120000
    }
  }
}

```

## Tools

| Tool | Description |
|------|-------------|
| `read_notebook` | Read notebook to LLM-friendly markdown |
| `write_notebook` | Write notebook from LLM-friendly markdown |
| `list_cells` | List all cells with IDs and types |
| `get_cell` | Get specific cell content |
| `add_cell` | Add new cell |
| `update_cell` | Update cell source |
| `delete_cell` | Delete cell by ID |
| `execute_notebook` | Execute whole notebook in-place via `jupyter nbconvert --execute` |
| `execute_cell` | Execute single code cell statelessly, outputs written back in-place |

Requires `jupyter` + `nbconvert` + a kernel (e.g. `pip install notebook nbconvert ipykernel`) in PATH for the two `execute_*` tools.
