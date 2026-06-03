//! Rust Component Model capsule builder.

use crate::archiver::pack_capsule_archive;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Build a Rust Component Model capsule from a crate directory.
///
/// # Errors
///
/// Returns an error when Cargo metadata cannot be read, the `wasm32-wasip2`
/// target is missing, compilation fails, the artifact is not a Component Model
/// binary, or archive packaging fails.
pub(crate) fn build(dir: &Path, output: Option<&str>) -> Result<()> {
    info!(
        "Building Rust Component Model capsule from {}",
        dir.display()
    );
    ensure_cargo_available()?;
    ensure_wasip2_target()?;

    let meta = cargo_metadata::MetadataCommand::new()
        .current_dir(dir)
        .no_deps()
        .exec()
        .context("Failed to parse Cargo metadata")?;

    let package = meta
        .packages
        .iter()
        .find(|p| {
            if let Some(parent) = p.manifest_path.parent()
                && let Ok(canon_parent) = parent.as_std_path().canonicalize()
                && let Ok(canon_dir) = dir.canonicalize()
            {
                return canon_parent == canon_dir;
            }
            false
        })
        .or_else(|| meta.root_package())
        .context("No package found matching the target directory in Cargo.toml")?;

    let crate_name = package.name.clone();
    let wasm_name = crate_name.replace('-', "_");

    info!("   Compiling target wasm32-wasip2...");
    let status = std::process::Command::new("cargo")
        .current_dir(dir)
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .status()
        .context("Failed to spawn cargo build")?;

    if !status.success() {
        bail!(
            "Cargo build failed for wasm32-wasip2. Install the target with \
             `rustup target add wasm32-wasip2`, then rerun `astrid build --type rust-component`."
        );
    }

    let wasm_path =
        locate_wasm_artifact(dir, &meta.workspace_root.into_std_path_buf(), &wasm_name)?;
    ensure_component_model(&wasm_path)?;

    let manifest_path = dir.join("Capsule.toml");
    if !manifest_path.is_file() {
        bail!(
            "Rust Component Model capsules must provide Capsule.toml with a \
             [[component]] file matching `{wasm_name}.wasm`"
        );
    }
    let toml_content = fs::read_to_string(&manifest_path).context("Failed to read Capsule.toml")?;

    let out_dir = output
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?.join("dist"));
    if !out_dir.exists() {
        fs::create_dir_all(&out_dir)?;
    }

    let out_file = out_dir.join(format!("{crate_name}.capsule"));
    pack_capsule_archive(&out_file, &toml_content, Some(&wasm_path), dir, &[])?;

    info!(
        "Successfully built Rust Component Model capsule: {}",
        out_file.display()
    );
    Ok(())
}

fn ensure_cargo_available() -> Result<()> {
    std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .context("`cargo` is not installed or not in PATH. Rust compilation failed.")?;
    Ok(())
}

fn ensure_wasip2_target() -> Result<()> {
    let output = std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .context("Failed to query installed Rust targets with `rustup`")?;
    if !output.status.success() {
        bail!("Failed to query installed Rust targets with `rustup target list --installed`");
    }
    let installed = String::from_utf8_lossy(&output.stdout);
    if !installed.lines().any(|line| line.trim() == "wasm32-wasip2") {
        bail!(
            "Rust target wasm32-wasip2 is required for Component Model capsules. \
             Install it with `rustup target add wasm32-wasip2`."
        );
    }
    Ok(())
}

fn locate_wasm_artifact(dir: &Path, workspace_root: &Path, wasm_name: &str) -> Result<PathBuf> {
    let local = dir
        .join("target")
        .join("wasm32-wasip2")
        .join("release")
        .join(format!("{wasm_name}.wasm"));
    if local.exists() {
        return Ok(local);
    }

    let workspace = workspace_root
        .join("target")
        .join("wasm32-wasip2")
        .join("release")
        .join(format!("{wasm_name}.wasm"));
    if workspace.exists() {
        return Ok(workspace);
    }

    bail!(
        "Could not locate compiled Component Model WASM binary. Checked {} and {}",
        local.display(),
        workspace.display()
    );
}

fn ensure_component_model(wasm_path: &Path) -> Result<()> {
    let bytes = fs::read(wasm_path)
        .with_context(|| format!("Failed to read WASM artifact {}", wasm_path.display()))?;
    match wasmparser::Parser::new(0).parse_all(&bytes).next() {
        Some(Ok(wasmparser::Payload::Version {
            encoding: wasmparser::Encoding::Component,
            ..
        })) => Ok(()),
        Some(Ok(wasmparser::Payload::Version {
            encoding: wasmparser::Encoding::Module,
            ..
        })) => bail!(
            "{} is a core WASM module, not a Component Model binary. \
             Build with the wasm32-wasip2 target and wit-bindgen exports.",
            wasm_path.display()
        ),
        _ => bail!("{} is not a valid WASM component", wasm_path.display()),
    }
}
