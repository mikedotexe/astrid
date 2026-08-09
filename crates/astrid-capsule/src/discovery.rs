//! Capsule manifest discovery from standard locations.
//!
//! Scans well-known directories for `Capsule.toml` files, providing
//! the entry point for the Manifest-First architecture.

use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};

use tracing::{debug, info, warn};

use crate::error::{CapsuleError, CapsuleResult};
use crate::manifest::{CapsuleManifest, InterceptorDef, InterceptorExposure, TopicDirection};

#[derive(Debug, Default, serde::Deserialize)]
struct DeclarativeRoutes {
    #[serde(default)]
    publish: BTreeMap<String, DeclarativeRoute>,
    #[serde(default)]
    subscribe: BTreeMap<String, DeclarativeRoute>,
}

#[derive(Debug, serde::Deserialize)]
struct DeclarativeRoute {
    #[serde(default)]
    handler: Option<String>,
}

/// Standard capsule manifest file name.
pub(crate) const MANIFEST_FILE_NAME: &str = "Capsule.toml";

/// Check if a string is a valid namespace or interface name: `^[a-z][a-z0-9-]*$`.
fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Validate namespace and interface name identifiers for a manifest section.
fn validate_interface_identifiers<'a>(
    path: &Path,
    section: &str,
    namespace: &str,
    names: impl Iterator<Item = &'a String>,
) -> CapsuleResult<()> {
    if !is_valid_identifier(namespace) {
        return Err(CapsuleError::ManifestParseError {
            path: path.to_path_buf(),
            message: format!(
                "[{section}].{namespace}: invalid namespace \
                 (must match ^[a-z][a-z0-9-]*$)"
            ),
        });
    }
    for name in names {
        if !is_valid_identifier(name) {
            return Err(CapsuleError::ManifestParseError {
                path: path.to_path_buf(),
                message: format!(
                    "[{section}.{namespace}].{name}: invalid interface name \
                     (must match ^[a-z][a-z0-9-]*$)"
                ),
            });
        }
    }
    Ok(())
}

/// Discover capsule manifests from standard locations with precedence.
///
/// Scans directories in priority order:
/// 1. `extra_paths` (system and principal capsule dirs, passed by kernel)
/// 2. `.astrid/capsules/` (workspace-level, relative to CWD)
///
/// **Deduplication:** When the same `package.name` appears in multiple
/// sources, the first occurrence wins (highest priority). Lower-priority
/// duplicates are logged as warnings and skipped. The kernel passes paths
/// in order: system (`~/.astrid/capsules/`), principal
/// (`~/.astrid/home/{id}/.local/capsules/`), then workspace is scanned
/// last.
///
/// Returns `(manifest, capsule_dir)` pairs where `capsule_dir` is the
/// directory containing the manifest.
pub fn discover_manifests(extra_paths: Option<&[PathBuf]>) -> Vec<(CapsuleManifest, PathBuf)> {
    let mut manifests = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    // Helper: load from a directory and deduplicate by name.
    let mut load_dedup = |dir: &Path, source: &str| {
        if !dir.exists() {
            return;
        }
        info!(path = %dir.display(), source, "Discovering capsules");
        match load_manifests_from_dir(dir) {
            Ok(found) => {
                for (manifest, path) in found {
                    if seen_names.contains(&manifest.package.name) {
                        warn!(
                            capsule = %manifest.package.name,
                            source,
                            skipped_path = %path.display(),
                            "Skipping duplicate capsule (higher-priority version already loaded)"
                        );
                    } else {
                        seen_names.insert(manifest.package.name.clone());
                        manifests.push((manifest, path));
                    }
                }
            },
            Err(e) => warn!(source, error = %e, "Failed to load capsules"),
        }
    };

    // 1. Extra paths in priority order (system, then principal).
    if let Some(paths) = extra_paths {
        for path in paths {
            load_dedup(path, "extra");
        }
    }

    // 2. Workspace-level capsules (lowest priority).
    load_dedup(&PathBuf::from(".astrid/capsules"), "workspace");

    info!(count = manifests.len(), "Discovered capsule manifests");
    manifests
}

/// Load all capsule manifests from a directory.
///
/// Looks for subdirectories containing `Capsule.toml` files, as well as
/// `Capsule.toml` files directly in the directory.
pub(crate) fn load_manifests_from_dir(
    dir: &Path,
) -> CapsuleResult<Vec<(CapsuleManifest, PathBuf)>> {
    let mut manifests = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(|e| CapsuleError::ManifestParseError {
        path: dir.to_path_buf(),
        message: e.to_string(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| CapsuleError::ManifestParseError {
            path: dir.to_path_buf(),
            message: e.to_string(),
        })?;
        let path = entry.path();

        if path.is_dir() {
            // Look for Capsule.toml in subdirectory
            let manifest_path = path.join(MANIFEST_FILE_NAME);
            if manifest_path.exists() {
                match load_manifest(&manifest_path) {
                    Ok(manifest) => {
                        debug!(
                            path = %manifest_path.display(),
                            capsule_name = %manifest.package.name,
                            "Loaded capsule manifest"
                        );
                        manifests.push((manifest, path));
                    },
                    Err(e) => {
                        warn!(
                            path = %manifest_path.display(),
                            error = %e,
                            "Failed to load capsule manifest"
                        );
                    },
                }
            }
        } else if path.is_file()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == MANIFEST_FILE_NAME)
        {
            let capsule_dir = path.parent().unwrap_or(dir).to_path_buf();
            match load_manifest(&path) {
                Ok(manifest) => {
                    debug!(
                        path = %path.display(),
                        capsule_name = %manifest.package.name,
                        "Loaded capsule manifest"
                    );
                    manifests.push((manifest, capsule_dir));
                },
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "Failed to load capsule manifest");
                },
            }
        }
    }

    Ok(manifests)
}

/// Load a single capsule manifest from a TOML file.
pub fn load_manifest(path: &Path) -> CapsuleResult<CapsuleManifest> {
    let content = std::fs::read_to_string(path).map_err(|e| CapsuleError::ManifestParseError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    let mut manifest: CapsuleManifest =
        toml::from_str(&content).map_err(|e| CapsuleError::ManifestParseError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

    merge_declarative_routes(path, &content, &mut manifest)?;

    // Merge component-level capabilities into the root capabilities.
    // [[component]].capabilities can declare fs_read, fs_write, host_process,
    // etc. These must be visible in the root `manifest.capabilities` because
    // the security gate reads from there.
    for component in &manifest.components {
        if let Some(ref caps) = component.capabilities {
            manifest.capabilities.fs_read.extend(caps.fs_read.clone());
            manifest.capabilities.fs_write.extend(caps.fs_write.clone());
            manifest
                .capabilities
                .host_process
                .extend(caps.host_process.clone());
            manifest.capabilities.net.extend(caps.net.clone());
            manifest.capabilities.net_bind.extend(caps.net_bind.clone());
        }
    }

    // Enforce astrid-version (MSRV for Astrid, like rust-version in Cargo.toml).
    // If the capsule requires a newer runtime than we are, reject it.
    // CARGO_PKG_VERSION is a compile-time constant; parse is trivially cheap.
    if let Some(ref constraint) = manifest.package.astrid_version {
        let runtime = semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("valid semver");
        let req = semver::VersionReq::parse(constraint).map_err(|e| {
            CapsuleError::ManifestParseError {
                path: path.to_path_buf(),
                message: format!("invalid astrid-version '{constraint}' - {e}"),
            }
        })?;

        if !req.matches(&runtime) {
            return Err(CapsuleError::ManifestParseError {
                path: path.to_path_buf(),
                message: format!(
                    "capsule requires astrid-version {constraint}, \
                     but this runtime is {runtime}"
                ),
            });
        }
    }

    // Validate version is valid semver (same as Cargo.toml).
    if semver::Version::parse(&manifest.package.version).is_err() {
        return Err(CapsuleError::ManifestParseError {
            path: path.to_path_buf(),
            message: format!(
                "invalid version '{}' in [package] - must be valid semver (MAJOR.MINOR.PATCH)",
                manifest.package.version
            ),
        });
    }

    // Validate IPC ACL and interceptor patterns for empty segments.
    let publish_patterns = manifest
        .capabilities
        .ipc_publish
        .iter()
        .map(|p| ("ipc_publish pattern", p.as_str()));
    let subscribe_patterns = manifest
        .capabilities
        .ipc_subscribe
        .iter()
        .map(|p| ("ipc_subscribe pattern", p.as_str()));
    let interceptor_patterns = manifest
        .interceptors
        .iter()
        .map(|i| ("interceptor event pattern", i.event.as_str()));

    for (kind, pattern) in publish_patterns
        .chain(subscribe_patterns)
        .chain(interceptor_patterns)
    {
        if !crate::topic::has_valid_segments(pattern) {
            return Err(CapsuleError::ManifestParseError {
                path: path.to_path_buf(),
                message: format!(
                    "{kind} '{pattern}' contains empty segments \
                     (consecutive dots, leading/trailing dots, or is empty)"
                ),
            });
        }
    }

    // Private tool routes are a host authorization surface, not merely a
    // prompt-hiding hint. Require an exact producer class and at least one
    // non-nil UUID source so an omitted configuration can only fail closed.
    for interceptor in &manifest.interceptors {
        if interceptor.exposure != InterceptorExposure::Private {
            continue;
        }
        if !interceptor.event.starts_with("tool.v1.") {
            return Err(CapsuleError::ManifestParseError {
                path: path.to_path_buf(),
                message: format!(
                    "private interceptor '{}' must be a tool.v1 route",
                    interceptor.event
                ),
            });
        }
        if interceptor
            .caller_producer_kind
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
            || interceptor.caller_source_ids.is_empty()
        {
            return Err(CapsuleError::ManifestParseError {
                path: path.to_path_buf(),
                message: format!(
                    "private interceptor '{}' requires an exact producer kind and source UUID",
                    interceptor.event
                ),
            });
        }
        for source_id in &interceptor.caller_source_ids {
            let parsed =
                uuid::Uuid::parse_str(source_id).map_err(|_| CapsuleError::ManifestParseError {
                    path: path.to_path_buf(),
                    message: format!(
                        "private interceptor '{}' has an invalid source UUID",
                        interceptor.event
                    ),
                })?;
            if parsed.is_nil() {
                return Err(CapsuleError::ManifestParseError {
                    path: path.to_path_buf(),
                    message: format!(
                        "private interceptor '{}' may not admit the nil source UUID",
                        interceptor.event
                    ),
                });
            }
        }
    }

    // Validate [imports] and [exports] namespace/name format.
    // Semver parsing is already handled by the custom Deserialize impls.
    for (namespace, ifaces) in &manifest.imports {
        validate_interface_identifiers(path, "imports", namespace, ifaces.keys())?;
    }
    for (namespace, ifaces) in &manifest.exports {
        validate_interface_identifiers(path, "exports", namespace, ifaces.keys())?;
    }

    // Uplink capsules load in a partition before non-uplinks.
    // Declaring [imports] on an uplink would violate this ordering.
    if manifest.capabilities.uplink && manifest.has_imports() {
        return Err(CapsuleError::ManifestParseError {
            path: path.to_path_buf(),
            message: "[imports] is not allowed on uplink capsules \
                      (uplinks load before non-uplinks and cannot depend on them)"
                .into(),
        });
    }

    // Validate [[topic]] declarations (structural only - no filesystem access).
    {
        let mut seen_topics: HashSet<(&str, TopicDirection)> = HashSet::new();
        for topic in &manifest.topics {
            // Topic name must have valid segments (no empty segments).
            if !crate::topic::has_valid_segments(&topic.name) {
                return Err(CapsuleError::ManifestParseError {
                    path: path.to_path_buf(),
                    message: format!(
                        "[[topic]] name '{}' contains empty segments \
                         (consecutive dots, leading/trailing dots, or is empty)",
                        topic.name
                    ),
                });
            }

            // Topic names must contain only alphanumeric, hyphens, underscores, and dots.
            // This implicitly rejects wildcards (*) and other special characters.
            if !topic
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                // Provide a specific message for wildcards since that's a common mistake.
                if topic.name.contains('*') {
                    return Err(CapsuleError::ManifestParseError {
                        path: path.to_path_buf(),
                        message: format!(
                            "[[topic]] name '{}' must be a concrete topic name, not a pattern \
                             (wildcards are not allowed in topic declarations)",
                            topic.name
                        ),
                    });
                }
                return Err(CapsuleError::ManifestParseError {
                    path: path.to_path_buf(),
                    message: format!(
                        "[[topic]] name '{}' contains invalid characters \
                         (only alphanumeric, hyphens, underscores, and dots are allowed)",
                        topic.name
                    ),
                });
            }

            // Schema path must not escape the capsule directory.
            if let Some(ref schema_path) = topic.schema {
                if schema_path.is_absolute() {
                    return Err(CapsuleError::ManifestParseError {
                        path: path.to_path_buf(),
                        message: format!(
                            "[[topic]] '{}' schema path must be relative, got absolute path '{}'",
                            topic.name,
                            schema_path.display()
                        ),
                    });
                }
                if schema_path
                    .components()
                    .any(|c| matches!(c, Component::ParentDir))
                {
                    return Err(CapsuleError::ManifestParseError {
                        path: path.to_path_buf(),
                        message: format!(
                            "[[topic]] '{}' schema path must not contain '..' components: '{}'",
                            topic.name,
                            schema_path.display()
                        ),
                    });
                }
            }

            // No duplicate (name, direction) pairs.
            if !seen_topics.insert((&topic.name, topic.direction)) {
                return Err(CapsuleError::ManifestParseError {
                    path: path.to_path_buf(),
                    message: format!(
                        "[[topic]] duplicate declaration: '{}' with direction '{}'",
                        topic.name, topic.direction
                    ),
                });
            }
        }
    }

    Ok(manifest)
}

/// Merge Cargo-like `[publish]` and `[subscribe]` route declarations into the
/// runtime structures used by Astrid 0.5.
///
/// Astralis capsules briefly shipped both manifest dialects during the
/// Component Model transition. The route tables are authoritative for IPC ACLs;
/// a subscribe route with a `handler` also declares an interceptor.
fn merge_declarative_routes(
    path: &Path,
    content: &str,
    manifest: &mut CapsuleManifest,
) -> CapsuleResult<()> {
    let routes: DeclarativeRoutes =
        toml::from_str(content).map_err(|e| CapsuleError::ManifestParseError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

    for topic in routes.publish.keys() {
        if !manifest.capabilities.ipc_publish.contains(topic) {
            manifest.capabilities.ipc_publish.push(topic.clone());
        }
    }

    for (topic, route) in routes.subscribe {
        if !manifest.capabilities.ipc_subscribe.contains(&topic) {
            manifest.capabilities.ipc_subscribe.push(topic.clone());
        }

        let Some(handler) = route.handler else {
            continue;
        };
        if handler.trim().is_empty() {
            return Err(CapsuleError::ManifestParseError {
                path: path.to_path_buf(),
                message: format!("[subscribe].'{topic}' handler must not be empty"),
            });
        }
        if !manifest
            .interceptors
            .iter()
            .any(|existing| existing.event == topic && existing.action == handler)
        {
            manifest.interceptors.push(InterceptorDef {
                event: topic,
                action: handler,
                priority: 100,
                exposure: crate::manifest::InterceptorExposure::Model,
                caller_producer_kind: None,
                caller_source_ids: Vec::new(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Write a TOML string to a temp file and call `load_manifest`.
    fn load_from_toml(toml: &str) -> CapsuleResult<crate::manifest::CapsuleManifest> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Capsule.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(toml.as_bytes()).unwrap();
        load_manifest(&path)
    }

    const VALID_HEADER: &str = r#"
[package]
name = "test-capsule"
version = "0.1.0"
"#;

    #[test]
    fn load_manifest_accepts_valid_ipc_publish() {
        let toml = format!(
            "{VALID_HEADER}\n[capabilities]\nipc_publish = [\"registry.*\", \"llm.stream.anthropic\"]"
        );
        assert!(load_from_toml(&toml).is_ok());
    }

    #[test]
    fn load_manifest_rejects_empty_segment_in_ipc_publish() {
        for bad in &["a..b", ".a.b", "a.b.", "", ".", "a...b"] {
            let toml = format!("{VALID_HEADER}\n[capabilities]\nipc_publish = [\"{bad}\"]");
            let err = load_from_toml(&toml).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("empty segments"),
                "expected 'empty segments' error for pattern '{bad}', got: {msg}"
            );
        }
    }

    #[test]
    fn load_manifest_rejects_empty_segment_in_interceptor_event() {
        for bad in &["a..b", ".event", "event.", "", ".", "a...b"] {
            let toml =
                format!("{VALID_HEADER}\n[[interceptor]]\nevent = \"{bad}\"\naction = \"handle\"");
            let err = load_from_toml(&toml).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("empty segments"),
                "expected 'empty segments' error for event '{bad}', got: {msg}"
            );
        }
    }

    #[test]
    fn load_manifest_accepts_valid_interceptor_event() {
        let toml = format!(
            "{VALID_HEADER}\n[[interceptor]]\nevent = \"user.prompt\"\naction = \"handle\""
        );
        assert!(load_from_toml(&toml).is_ok());
    }

    #[test]
    fn private_tool_interceptor_requires_exact_nonempty_caller_policy() {
        let valid = format!(
            "{VALID_HEADER}\n\
             [[interceptor]]\n\
             event = \"tool.v1.execute.inspect_owned_question\"\n\
             action = \"inspect\"\n\
             exposure = \"private\"\n\
             caller_producer_kind = \"native_socket_client\"\n\
             caller_source_ids = [\"a57d1d30-0000-4000-8000-000000000001\"]"
        );
        let manifest = load_from_toml(&valid).unwrap();
        assert_eq!(
            manifest.interceptors[0].exposure,
            InterceptorExposure::Private
        );

        for invalid_fields in [
            "",
            "caller_producer_kind = \"native_socket_client\"",
            "caller_source_ids = [\"a57d1d30-0000-4000-8000-000000000001\"]",
            "caller_producer_kind = \"native_socket_client\"\ncaller_source_ids = [\"not-a-uuid\"]",
            "caller_producer_kind = \"native_socket_client\"\ncaller_source_ids = [\"00000000-0000-0000-0000-000000000000\"]",
        ] {
            let invalid = format!(
                "{VALID_HEADER}\n\
                 [[interceptor]]\n\
                 event = \"tool.v1.execute.inspect_owned_question\"\n\
                 action = \"inspect\"\n\
                 exposure = \"private\"\n{invalid_fields}"
            );
            assert!(
                load_from_toml(&invalid).is_err(),
                "accepted {invalid_fields:?}"
            );
        }
    }

    #[test]
    fn load_manifest_accepts_valid_semver() {
        let toml = "[package]\nname = \"test\"\nversion = \"1.2.3\"\n";
        assert!(load_from_toml(toml).is_ok());
    }

    #[test]
    fn load_manifest_accepts_prerelease_semver() {
        let toml = "[package]\nname = \"test\"\nversion = \"1.0.0-alpha.1\"\n";
        assert!(load_from_toml(toml).is_ok());
    }

    #[test]
    fn load_manifest_rejects_incomplete_semver() {
        let toml = "[package]\nname = \"test\"\nversion = \"1.0\"\n";
        let err = load_from_toml(toml).unwrap_err();
        assert!(
            err.to_string().contains("invalid version"),
            "expected 'invalid version' error, got: {err}"
        );
    }

    #[test]
    fn load_manifest_rejects_non_semver_version() {
        let toml = "[package]\nname = \"test\"\nversion = \"latest\"\n";
        let err = load_from_toml(toml).unwrap_err();
        assert!(
            err.to_string().contains("invalid version"),
            "expected 'invalid version' error, got: {err}"
        );
    }

    #[test]
    fn load_manifest_parses_imports_and_exports() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [imports.astrid]\n\
             llm = \"^1.0\"\n\
             session = {{ version = \"^1.0\", optional = true }}\n\n\
             [exports.astrid]\n\
             identity = \"1.0.0\"\n"
        );
        let m = load_from_toml(&toml).unwrap();
        let astrid_imports = m.imports.get("astrid").unwrap();
        assert_eq!(astrid_imports.len(), 2);
        assert!(!astrid_imports["llm"].optional);
        assert!(astrid_imports["session"].optional);
        let astrid_exports = m.exports.get("astrid").unwrap();
        assert_eq!(astrid_exports.len(), 1);
        assert_eq!(
            astrid_exports["identity"].version,
            semver::Version::new(1, 0, 0)
        );
    }

    #[test]
    fn load_manifest_parses_legacy_flat_imports_and_exports() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [imports]\n\
             \"astrid:llm\" = \"^1.0\"\n\
             \"astrid:session\" = {{ version = \"^1.0\", optional = true }}\n\n\
             [exports]\n\
             \"astrid:spark\" = \"1.0.0\"\n"
        );
        let m = load_from_toml(&toml).unwrap();

        let astrid_imports = m.imports.get("astrid").unwrap();
        assert_eq!(astrid_imports.len(), 2);
        assert!(!astrid_imports["llm"].optional);
        assert!(astrid_imports["session"].optional);

        let astrid_exports = m.exports.get("astrid").unwrap();
        assert_eq!(
            astrid_exports["spark"].version,
            semver::Version::new(1, 0, 0)
        );
    }

    #[test]
    fn load_manifest_rejects_flat_interface_without_namespace() {
        let toml = format!("{VALID_HEADER}\n[exports]\nspark = \"1.0.0\"");
        assert!(load_from_toml(&toml).is_err());
    }

    #[test]
    fn load_manifest_defaults_empty_imports_exports() {
        let m = load_from_toml(VALID_HEADER).unwrap();
        assert!(m.imports.is_empty());
        assert!(m.exports.is_empty());
        assert!(!m.has_imports());
    }

    #[test]
    fn load_manifest_parses_exports_only() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [exports.astrid]\n\
             session = \"1.0.0\"\n\
             context = {{ version = \"1.0.0\" }}\n"
        );
        let m = load_from_toml(&toml).unwrap();
        assert!(m.imports.is_empty());
        let astrid = m.exports.get("astrid").unwrap();
        assert_eq!(astrid.len(), 2);
    }

    #[test]
    fn load_manifest_rejects_invalid_namespace() {
        let toml = format!("{VALID_HEADER}\n[exports.INVALID]\nfoo = \"1.0.0\"");
        let err = load_from_toml(&toml).unwrap_err();
        assert!(
            err.to_string().contains("invalid namespace"),
            "expected 'invalid namespace' error, got: {err}"
        );
    }

    #[test]
    fn load_manifest_rejects_invalid_interface_name() {
        let toml = format!("{VALID_HEADER}\n[exports.astrid]\n\"BAD_NAME\" = \"1.0.0\"");
        let err = load_from_toml(&toml).unwrap_err();
        assert!(
            err.to_string().contains("invalid interface name"),
            "expected 'invalid interface name' error, got: {err}"
        );
    }

    #[test]
    fn load_manifest_rejects_invalid_import_version() {
        let toml = format!("{VALID_HEADER}\n[imports.astrid]\nllm = \"not_semver\"");
        let err = load_from_toml(&toml).unwrap_err();
        assert!(
            err.to_string().contains("invalid semver"),
            "expected semver error, got: {err}"
        );
    }

    #[test]
    fn load_manifest_rejects_invalid_export_version() {
        let toml = format!("{VALID_HEADER}\n[exports.astrid]\nllm = \"not_semver\"");
        let err = load_from_toml(&toml).unwrap_err();
        assert!(
            err.to_string().contains("invalid semver"),
            "expected semver error, got: {err}"
        );
    }

    #[test]
    fn load_manifest_rejects_uplink_with_imports() {
        let toml = format!(
            "{VALID_HEADER}\n[capabilities]\nuplink = true\n\n[imports.astrid]\nllm = \"^1.0\""
        );
        let err = load_from_toml(&toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("not allowed on uplink"),
            "expected uplink+imports rejection, got: {msg}"
        );
    }

    #[test]
    fn load_manifest_allows_uplink_without_imports() {
        let toml = format!("{VALID_HEADER}\n[capabilities]\nuplink = true");
        assert!(
            load_from_toml(&toml).is_ok(),
            "uplink without imports should be valid"
        );
    }

    #[test]
    fn load_manifest_merges_declarative_routes_into_runtime_acl_and_interceptors() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [capabilities]\n\
             ipc_publish = [\"existing.v1.event\"]\n\
             \n\
             [[interceptor]]\n\
             event = \"existing.v1.request\"\n\
             action = \"handle_existing\"\n\
             \n\
             [publish]\n\
             \"llm.v1.stream.provider\" = {{ wit = \"stream-event\" }}\n\
             \"existing.v1.event\" = {{ wit = \"event\" }}\n\
             \n\
             [subscribe]\n\
             \"llm.v1.request.generate.provider\" = {{ wit = \"request\", handler = \"handle_generate\" }}\n\
             \"registry.v1.*\" = {{ wit = \"TODO\" }}\n\
             \"existing.v1.request\" = {{ wit = \"request\", handler = \"handle_existing\" }}\n"
        );

        let manifest = load_from_toml(&toml).expect("declarative routes should load");
        assert_eq!(
            manifest.capabilities.ipc_publish,
            ["existing.v1.event", "llm.v1.stream.provider"]
        );
        assert_eq!(
            manifest.capabilities.ipc_subscribe,
            [
                "existing.v1.request",
                "llm.v1.request.generate.provider",
                "registry.v1.*",
            ]
        );
        assert_eq!(
            manifest
                .interceptors
                .iter()
                .filter(|entry| entry.event == "existing.v1.request")
                .count(),
            1,
            "an explicit interceptor and matching route must not duplicate"
        );
        assert!(manifest.interceptors.iter().any(|entry| {
            entry.event == "llm.v1.request.generate.provider"
                && entry.action == "handle_generate"
                && entry.priority == 100
        }));
        assert!(
            !manifest
                .interceptors
                .iter()
                .any(|entry| entry.event == "registry.v1.*"),
            "a handler-less subscription is an ACL declaration, not an interceptor"
        );
    }

    #[test]
    fn load_manifest_rejects_empty_declarative_route_handler() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [subscribe]\n\
             \"llm.v1.request\" = {{ handler = \"\" }}\n"
        );
        let err = load_from_toml(&toml).unwrap_err();
        assert!(
            err.to_string().contains("handler must not be empty"),
            "expected empty handler rejection, got: {err}"
        );
    }

    #[test]
    fn load_manifest_accepts_satisfied_astrid_version() {
        let toml = "[package]\nname = \"test\"\nversion = \"0.1.0\"\nastrid-version = \">=0.1.0\"";
        assert!(load_from_toml(toml).is_ok());
    }

    #[test]
    fn load_manifest_rejects_unsatisfied_astrid_version() {
        let toml = "[package]\nname = \"test\"\nversion = \"0.1.0\"\nastrid-version = \">=99.0.0\"";
        let err = load_from_toml(toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("astrid-version") && msg.contains("99.0.0"),
            "expected astrid-version rejection, got: {msg}"
        );
    }

    #[test]
    fn load_manifest_rejects_invalid_astrid_version() {
        let toml =
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nastrid-version = \"not-semver\"";
        let err = load_from_toml(toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid astrid-version"),
            "expected parse error, got: {msg}"
        );
    }

    #[test]
    fn load_manifest_accepts_missing_astrid_version() {
        // No astrid-version field at all - should load fine.
        assert!(load_from_toml(VALID_HEADER).is_ok());
    }

    // -----------------------------------------------------------------------
    // [[topic]] validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn topic_parses_valid_publish_and_subscribe() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [[topic]]\n\
             name = \"llm.v1.response.chunk\"\n\
             direction = \"publish\"\n\
             description = \"Streaming LLM response chunk\"\n\
             \n\
             [[topic]]\n\
             name = \"llm.v1.request.generate\"\n\
             direction = \"subscribe\"\n"
        );
        let manifest = load_from_toml(&toml).expect("valid topics");
        assert_eq!(manifest.topics.len(), 2);
        assert_eq!(manifest.topics[0].direction, TopicDirection::Publish);
        assert_eq!(manifest.topics[1].direction, TopicDirection::Subscribe);
    }

    #[test]
    fn topic_without_optional_fields() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [[topic]]\n\
             name = \"events.v1.notify\"\n\
             direction = \"publish\"\n"
        );
        let manifest = load_from_toml(&toml).expect("valid topic without optionals");
        assert_eq!(manifest.topics.len(), 1);
        assert!(manifest.topics[0].description.is_none());
        assert!(manifest.topics[0].schema.is_none());
    }

    #[test]
    fn topic_rejects_invalid_direction() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [[topic]]\n\
             name = \"foo.bar\"\n\
             direction = \"bidirectional\"\n"
        );
        let err = load_from_toml(&toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown variant"),
            "expected serde enum error, got: {msg}"
        );
    }

    #[test]
    fn topic_rejects_empty_segment_name() {
        for bad in &["a..b", ".a.b", "a.b.", "", "."] {
            let toml = format!(
                "{VALID_HEADER}\n\
                 [[topic]]\n\
                 name = \"{bad}\"\n\
                 direction = \"publish\"\n"
            );
            let err = load_from_toml(&toml).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("empty segments"),
                "expected 'empty segments' error for name '{bad}', got: {msg}"
            );
        }
    }

    #[test]
    fn topic_rejects_absolute_schema_path() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [[topic]]\n\
             name = \"foo.bar\"\n\
             direction = \"publish\"\n\
             schema = \"/etc/passwd\"\n"
        );
        let err = load_from_toml(&toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("must be relative"),
            "expected relative path error, got: {msg}"
        );
    }

    #[test]
    fn topic_rejects_parent_dir_in_schema_path() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [[topic]]\n\
             name = \"foo.bar\"\n\
             direction = \"publish\"\n\
             schema = \"../escape.json\"\n"
        );
        let err = load_from_toml(&toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("'..'"),
            "expected parent dir error, got: {msg}"
        );
    }

    #[test]
    fn topic_rejects_wildcard_segment_name() {
        for bad in &["llm.v1.*", "*.response", "a.*.b"] {
            let toml = format!(
                "{VALID_HEADER}\n\
                 [[topic]]\n\
                 name = \"{bad}\"\n\
                 direction = \"publish\"\n"
            );
            let err = load_from_toml(&toml).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("wildcard"),
                "expected wildcard error for name '{bad}', got: {msg}"
            );
        }
    }

    #[test]
    fn topic_rejects_invalid_characters() {
        for bad in &["llm response", "foo@bar", "a/b/c", "topic!bang"] {
            let toml = format!(
                "{VALID_HEADER}\n\
                 [[topic]]\n\
                 name = \"{bad}\"\n\
                 direction = \"publish\"\n"
            );
            let err = load_from_toml(&toml).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("invalid characters"),
                "expected invalid characters error for name '{bad}', got: {msg}"
            );
        }
    }

    #[test]
    fn topic_rejects_duplicate_name_direction_pair() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [[topic]]\n\
             name = \"foo.bar\"\n\
             direction = \"publish\"\n\
             \n\
             [[topic]]\n\
             name = \"foo.bar\"\n\
             direction = \"publish\"\n"
        );
        let err = load_from_toml(&toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("duplicate"),
            "expected duplicate error, got: {msg}"
        );
    }

    #[test]
    fn topic_allows_same_name_different_direction() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [[topic]]\n\
             name = \"echo.v1\"\n\
             direction = \"publish\"\n\
             \n\
             [[topic]]\n\
             name = \"echo.v1\"\n\
             direction = \"subscribe\"\n"
        );
        let manifest = load_from_toml(&toml).expect("same name different direction is valid");
        assert_eq!(manifest.topics.len(), 2);
    }

    #[test]
    fn topic_backwards_compat_no_topics_section() {
        // Existing manifests without [[topic]] must still parse.
        let manifest = load_from_toml(VALID_HEADER).expect("no topics section is fine");
        assert!(manifest.topics.is_empty());
    }

    #[test]
    fn topic_with_schema_path() {
        let toml = format!(
            "{VALID_HEADER}\n\
             [[topic]]\n\
             name = \"llm.v1.chunk\"\n\
             direction = \"publish\"\n\
             schema = \"schemas/chunk.json\"\n"
        );
        let manifest = load_from_toml(&toml).expect("schema path is valid");
        assert_eq!(
            manifest.topics[0].schema.as_deref(),
            Some(std::path::Path::new("schemas/chunk.json"))
        );
    }
}
