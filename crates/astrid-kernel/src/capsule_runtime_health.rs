use std::collections::HashSet;
use std::path::{Path, PathBuf};

use astrid_events::kernel_api::CapsuleRuntimeHealth;

#[derive(Debug, Default, serde::Deserialize)]
struct Baseline {
    #[serde(default)]
    accepted_legacy_extism_mvp: Vec<BaselineEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct BaselineEntry {
    name: String,
    #[serde(default)]
    wasm_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadKind {
    ComponentModel,
    LegacyExtismMvp,
    CoreModuleMvp,
    Invalid,
}

pub(crate) fn summarize(workspace_root: &Path, loaded_capsules: &[String]) -> CapsuleRuntimeHealth {
    let dirs = discovery_dirs(workspace_root);
    let installed_manifests = count_manifest_paths(&dirs);
    let discovered = discover_manifest_entries(&dirs);
    let baseline = load_baseline(workspace_root).unwrap_or_default();
    let home_bin = astrid_core::dirs::AstridHome::resolve()
        .ok()
        .map(|home| home.bin_dir());

    let mut health = CapsuleRuntimeHealth {
        status: "ok".to_string(),
        installed_manifests: to_u32(installed_manifests),
        discovered_manifests: to_u32(discovered.len()),
        loaded_capsules: to_u32(loaded_capsules.len()),
        ..CapsuleRuntimeHealth::default()
    };

    for (manifest, dir) in discovered {
        let Some(component) = manifest.components.first() else {
            continue;
        };
        let wasm_hash = read_meta_wasm_hash(&dir);
        let Some(payload_path) =
            resolve_component_payload(&dir, &component.path, home_bin.as_ref())
        else {
            health.missing_payloads = health.missing_payloads.saturating_add(1);
            health.actionable_missing_payloads =
                health.actionable_missing_payloads.saturating_add(1);
            continue;
        };

        health.component_payloads_found = health.component_payloads_found.saturating_add(1);
        match classify_payload(&payload_path) {
            PayloadKind::ComponentModel => {
                health.loadable_component_model = health.loadable_component_model.saturating_add(1);
            },
            PayloadKind::LegacyExtismMvp => {
                health.legacy_extism_mvp = health.legacy_extism_mvp.saturating_add(1);
                if accepted_legacy(&baseline, &manifest.package.name, wasm_hash.as_deref()) {
                    health.accepted_legacy_extism_mvp =
                        health.accepted_legacy_extism_mvp.saturating_add(1);
                } else {
                    health.actionable_incompatible =
                        health.actionable_incompatible.saturating_add(1);
                }
            },
            PayloadKind::CoreModuleMvp | PayloadKind::Invalid => {
                health.actionable_incompatible = health.actionable_incompatible.saturating_add(1);
            },
        }
    }

    if health.actionable_incompatible > 0 || health.actionable_missing_payloads > 0 {
        health.status = "warning".to_string();
    }
    health
}

fn discovery_dirs(workspace_root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = astrid_core::dirs::AstridHome::resolve() {
        let principal = astrid_core::PrincipalId::default();
        dirs.push(home.principal_home(&principal).capsules_dir());
    }
    dirs.push(workspace_root.join(".astrid").join("capsules"));
    dirs
}

fn count_manifest_paths(dirs: &[PathBuf]) -> usize {
    dirs.iter().map(|dir| manifest_paths(dir).len()).sum()
}

fn discover_manifest_entries(
    dirs: &[PathBuf],
) -> Vec<(astrid_capsule::manifest::CapsuleManifest, PathBuf)> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for manifest_path in dirs.iter().flat_map(|dir| manifest_paths(dir)) {
        let capsule_dir = manifest_path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        if let Ok(manifest) = astrid_capsule::discovery::load_manifest(&manifest_path)
            && seen.insert(manifest.package.name.clone())
        {
            entries.push((manifest, capsule_dir));
        }
    }
    entries
}

fn manifest_paths(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let direct = dir.join("Capsule.toml");
    if direct.is_file() {
        paths.push(direct);
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return paths;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest = path.join("Capsule.toml");
            if manifest.is_file() {
                paths.push(manifest);
            }
        }
    }
    paths.sort();
    paths
}

fn read_meta_wasm_hash(capsule_dir: &Path) -> Option<String> {
    let meta_path = capsule_dir.join("meta.json");
    let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(meta_path).ok()?).ok()?;
    meta.get("wasm_hash")?.as_str().map(String::from)
}

fn resolve_component_payload(
    capsule_dir: &Path,
    component_path: &Path,
    home_bin: Option<&PathBuf>,
) -> Option<PathBuf> {
    let local = if component_path.is_absolute() {
        component_path.to_path_buf()
    } else {
        capsule_dir.join(component_path)
    };
    if local.is_file() {
        return Some(local);
    }

    let hash = read_meta_wasm_hash(capsule_dir)?;
    let hashed = home_bin?.join(format!("{hash}.wasm"));
    hashed.is_file().then_some(hashed)
}

fn classify_payload(path: &Path) -> PayloadKind {
    let Ok(bytes) = std::fs::read(path) else {
        return PayloadKind::Invalid;
    };

    match wasmparser::Parser::new(0).parse_all(&bytes).next() {
        Some(Ok(wasmparser::Payload::Version {
            encoding: wasmparser::Encoding::Component,
            ..
        })) => PayloadKind::ComponentModel,
        Some(Ok(wasmparser::Payload::Version {
            encoding: wasmparser::Encoding::Module,
            ..
        })) => {
            if contains_extism_marker(&bytes) {
                PayloadKind::LegacyExtismMvp
            } else {
                PayloadKind::CoreModuleMvp
            }
        },
        _ => PayloadKind::Invalid,
    }
}

fn contains_extism_marker(bytes: &[u8]) -> bool {
    bytes
        .windows("extism".len())
        .any(|window| window.eq_ignore_ascii_case(b"extism"))
}

fn load_baseline(workspace_root: &Path) -> Option<Baseline> {
    let path = workspace_root
        .join("scripts")
        .join("baselines")
        .join("capsule_runtime_health.json");
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn accepted_legacy(baseline: &Baseline, name: &str, wasm_hash: Option<&str>) -> bool {
    baseline.accepted_legacy_extism_mvp.iter().any(|entry| {
        entry.name == name
            && match entry.wasm_hash.as_deref() {
                Some(expected) => wasm_hash == Some(expected),
                None => true,
            }
    })
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
