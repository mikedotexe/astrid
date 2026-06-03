# astrid-build

[![Crates.io](https://img.shields.io/crates/v/astrid-build)](https://crates.io/crates/astrid-build)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](../../LICENSE-MIT)
[![MSRV: 1.94](https://img.shields.io/badge/MSRV-1.94-blue)](https://www.rust-lang.org)

**Capsule compilation and packaging for the Astrid OS.**

Compiles Rust, OpenClaw (JS/TS), and legacy MCP server projects into `.capsule` archives that the Astrid kernel can load. Typically invoked by the CLI (`astrid build`) but can be used standalone. The runnable `astrid-build` companion binary is shipped by the `astrid` release package; this crate remains the build library/API surface for workspace builds and tests.

## Usage

### Via the CLI (typical)

```bash
# Auto-detect project type and build
astrid build

# Specify output directory
astrid build --output ./dist

# Build a specific project directory
astrid build /path/to/capsule

# Build a Rust Component Model capsule
astrid build /path/to/capsule --type rust-component

# Convert a legacy MCP server manifest
astrid build --from-mcp-json mcp.json
```

### Standalone

```bash
astrid-build [PATH] [OPTIONS]
```

## Flags

| Flag | Description |
|---|---|
| `[PATH]` | Project directory (defaults to current directory) |
| `-o, --output <DIR>` | Output directory for the `.capsule` archive |
| `-t, --type <TYPE>` | Explicit project type: `rust`, `rust-component`, `openclaw`, `mcp`, `extension` |
| `--from-mcp-json <FILE>` | Import a legacy `mcp.json` or `gemini-extension.json` to auto-convert |

## Supported project types

| Type | Detection | What happens |
|---|---|---|
| **Rust Component** | explicit `--type rust-component` | `cargo build --target wasm32-wasip2 --release`, verifies the artifact is a Component Model binary, packs `Capsule.toml` + WASM |
| **Legacy Rust** | `Cargo.toml` | `cargo build --target wasm32-wasip1 --release`, merges `Capsule.toml`, packs archive |
| **OpenClaw** | `openclaw.plugin.json` | Transpiles JS/TS via the OpenClaw pipeline (Tier 1 WASM or Tier 2 Node.js), packs archive |
| **MCP** | `mcp.json` | Converts legacy MCP server manifest to `Capsule.toml`, packs archive |
| **Extension** | `gemini-extension.json` | Same as MCP, for Gemini extension format |

## Output

A `.capsule` file — a gzipped tar archive containing:

- `Capsule.toml` — manifest with package metadata, capabilities, tool schemas
- `*.wasm` — the compiled WASM binary (Rust Component, legacy Rust, and Tier 1 OpenClaw)
- `node_modules/` + source — for Tier 2 OpenClaw (Node.js runtime)

Install the built capsule:

```bash
astrid capsule install ./my-capsule.capsule
```

## Development

```bash
cargo build -p astrid --release
cargo test -p astrid-build
```

The bundled release path is `cargo build -p astrid --release`, which produces
the `astrid`, `astrid-daemon`, and `astrid-build` companion binaries together.

## License

Dual MIT/Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and [LICENSE-APACHE](../../LICENSE-APACHE).
