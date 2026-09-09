//! A mechanical reading view of saved terminal output; raw evidence stays intact.
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewManifest {
    schema: String,
    raw_sha256: String,
    view_sha256: String,
}

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn write_new(path: &Path, text: &str) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Retain the raw overflow and, only when necessary, a separately identified
/// reading view. Failure never advertises a half-written readable view.
pub(super) fn save(raw_path: &Path, raw: &str) -> Result<PathBuf> {
    write_new(raw_path, raw)?;
    let body = readable_body(raw);
    if body == raw {
        return Ok(raw_path.to_path_buf());
    }
    let view_path = raw_path.with_extension("readable.txt");
    let view = render(raw_path, raw, &body)?;
    write_new(&view_path, &view)?;
    let manifest = ViewManifest {
        schema: "prompt_overflow_reading_view_v1".into(),
        raw_sha256: digest(raw),
        view_sha256: digest(&view),
    };
    write_new(
        &view_path.with_extension("json"),
        &serde_json::to_string(&manifest)?,
    )?;
    fs::File::open(raw_path.parent().context("overflow directory")?)?.sync_all()?;
    Ok(view_path)
}

fn render(raw_path: &Path, raw: &str, body: &str) -> Result<String> {
    Ok(format!(
        "Saved prompt overflow — readable view. Terminal control encoding removed only from the spectral section; visible glyphs and section order retained. This is a linear text view, not a terminal screen reconstruction.\nRaw origin: {}; sha256:{}; {} bytes. This view has its own revision and byte positions. READ_MORE RAW selects the retained original; RETURN_ACTIVITY returns to this view.\n\n{body}",
        serde_json::to_string(&raw_path.canonicalize()?)?,
        digest(raw),
        raw.len()
    ))
}

/// Bind a new view to its actual raw sibling and deterministic transformation.
/// A filename, prose label or editable sidecar alone cannot establish provenance.
pub(crate) fn verified_raw_source(
    view_path: &Path,
    view: &str,
) -> Result<Option<(PathBuf, String)>> {
    let Some(stem) = view_path
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_suffix(".readable.txt"))
        .filter(|s| s.starts_with("context_overflow_"))
    else {
        return Ok(None);
    };
    let manifest_path = view_path.with_extension("json");
    if fs::metadata(&manifest_path)?.len() > 4096 {
        bail!("reading view manifest exceeds its bound");
    }
    let manifest: ViewManifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
    let raw_path = view_path.with_file_name(format!("{stem}.txt"));
    if fs::metadata(&raw_path)?.len() > 64 * 1024 * 1024 {
        bail!("raw overflow exceeds the source bound");
    }
    let raw = fs::read_to_string(&raw_path)?;
    if manifest.schema != "prompt_overflow_reading_view_v1"
        || manifest.raw_sha256 != digest(&raw)
        || manifest.view_sha256 != digest(view)
        || render(&raw_path, &raw, &readable_body(&raw))? != view
    {
        bail!("reading view does not match its raw origin and transformation");
    }
    Ok(Some((raw_path, manifest.raw_sha256)))
}

/// Only the runtime's terminal-rendered spectral section is transformed.
/// Source code, letters, peer journals and other saved sections remain exact.
fn readable_body(raw: &str) -> String {
    let mut terminal = false;
    let mut output = String::new();
    let mut section = String::new();
    for line in raw.split_inclusive('\n') {
        if line.starts_with("=== [") && line.trim_end().ends_with("] ===") {
            if terminal {
                output.push_str(&strip_terminal_controls(&section));
            } else {
                output.push_str(&section);
            }
            section.clear();
            terminal = line.trim_end() == "=== [spectral] ===";
        }
        section.push_str(line);
    }
    if terminal {
        output.push_str(&strip_terminal_controls(&section));
    } else {
        output.push_str(&section);
    }
    output
}

/// Consume CSI, OSC and other ECMA-48 escape/control strings. No cursor motion
/// is executed and no printable glyph is interpreted as markup or instructions.
fn strip_terminal_controls(text: &str) -> String {
    let mut chars = text.chars().peekable();
    let mut output = String::new();
    while let Some(ch) = chars.next() {
        let introducer = if ch == '\u{1b}' {
            chars.next()
        } else {
            Some(ch)
        };
        match introducer {
            Some('[') if ch == '\u{1b}' => consume_csi(&mut chars),
            Some('\u{9b}') => consume_csi(&mut chars),
            Some(']' | 'P' | 'X' | '^' | '_') if ch == '\u{1b}' => consume_string(&mut chars),
            Some('\u{90}' | '\u{98}' | '\u{9d}' | '\u{9e}' | '\u{9f}') => {
                consume_string(&mut chars)
            },
            Some(c) if ch == '\u{1b}' => {
                if (' '..='/').contains(&c) {
                    for c in chars.by_ref() {
                        if !(' '..='/').contains(&c) {
                            break;
                        }
                    }
                }
            },
            Some(c) if !c.is_control() || matches!(c, '\n' | '\t') => output.push(c),
            _ => {},
        }
    }
    output
}

fn consume_csi(chars: &mut impl Iterator<Item = char>) {
    for c in chars.by_ref() {
        if ('@'..='~').contains(&c) {
            break;
        }
    }
}

fn consume_string(chars: &mut impl Iterator<Item = char>) {
    let mut escaped = false;
    for c in chars.by_ref() {
        if matches!(c, '\u{7}' | '\u{9c}') || (escaped && c == '\\') {
            break;
        }
        escaped = c == '\u{1b}';
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_control_strings_can_span_lines_without_leaking_payload() {
        assert_eq!(
            readable_body(
                "=== [spectral] ===\nλ\u{1b}]title\nprivate title\u{1b}\\░\n=== [journal] ===\nexact\n"
            ),
            "=== [spectral] ===\nλ░\n=== [journal] ===\nexact\n"
        );
    }

    #[test]
    fn views_retain_glyphs_and_raw_bytes_without_touching_other_sections() {
        let root = tempfile::tempdir().unwrap();
        let raw_path = root.path().join("context_overflow_1_1.txt");
        let raw = "=== [spectral] ===\n\n\u{1b}[38;2;100;50;30mλ ░🙂\u{1b}[0m\u{1b}]title\u{7}\n\n=== [journal] ===\n\nExact letter \u{1b}[31mcode\u{1b}[0m\n\n";
        let view_path = save(&raw_path, raw).unwrap();
        let view = fs::read_to_string(&view_path).unwrap();
        assert_eq!(fs::read_to_string(&raw_path).unwrap(), raw);
        assert!(view.ends_with("=== [spectral] ===\n\nλ ░🙂\n\n=== [journal] ===\n\nExact letter \u{1b}[31mcode\u{1b}[0m\n\n"));
        assert!(view.contains("own revision and byte positions"));
        assert_ne!(digest(raw), digest(&view));
        assert_eq!(
            verified_raw_source(&view_path, &view).unwrap(),
            Some((raw_path.clone(), digest(raw)))
        );
        fs::write(&raw_path, "changed origin").unwrap();
        assert!(verified_raw_source(&view_path, &view).is_err());
    }

    #[test]
    fn relabelled_view_is_rejected_even_with_matching_sidecar_hash() {
        let root = tempfile::tempdir().unwrap();
        let raw_path = root.path().join("context_overflow_1_1.txt");
        let raw = "=== [spectral] ===\n\n\u{1b}[31mvisible\u{1b}[0m\n\n";
        let view_path = save(&raw_path, raw).unwrap();
        let forged = fs::read_to_string(&view_path)
            .unwrap()
            .replace("Raw origin:", "Current verified code:");
        let sidecar = ViewManifest {
            schema: "prompt_overflow_reading_view_v1".into(),
            raw_sha256: digest(raw),
            view_sha256: digest(&forged),
        };
        fs::write(
            view_path.with_extension("json"),
            serde_json::to_vec(&sidecar).unwrap(),
        )
        .unwrap();
        assert!(verified_raw_source(&view_path, &forged).is_err());
    }

    #[test]
    fn exact_text_without_terminal_spectral_output_has_no_new_view() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("context_overflow_1_1.txt");
        let source = "=== [journal] ===\n\nCode: \u{1b}[31mexample\n\n";
        assert_eq!(save(&path, source).unwrap(), path);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        assert!(verified_raw_source(&path, source).unwrap().is_none());
    }

    // The fixtures contain private journal/runtime material and intentionally
    // remain in the research capture, not the source repository. Body digests
    // were computed independently by removing complete ANSI CSI sequences.
    #[test]
    #[ignore = "requires ASTRID_READING_FIXTURE_DIR pointing to the retained S-008 supplement"]
    fn retained_s008_overflow_documents() {
        let fixtures = PathBuf::from(std::env::var("ASTRID_READING_FIXTURE_DIR").unwrap());
        let cases = [
            (
                "context_overflow_84971_1788914807621896000.txt",
                "93feb624672edc45488e40a4e940f914d04774e3903435d7f1514805af1c17ff",
                "4967410a5322d92191a59f627489835fbd1b7955754a1988314a122a324a137b",
                17121,
            ),
            (
                "context_overflow_84971_1788915723273931000.txt",
                "68bbd9828ea558c03497867edea280f82274a20c0542f163413dcabf3ec507de",
                "591f28265fae56a0023f599b70791cb09895ae89bfdf8973c2a3faafde300e00",
                19066,
            ),
        ];
        for (name, raw_hash, body_hash, body_bytes) in cases {
            let raw = fs::read_to_string(fixtures.join(name)).unwrap();
            assert_eq!(digest(&raw), raw_hash);
            let body = readable_body(&raw);
            assert_eq!(digest(&body), body_hash);
            assert_eq!(body.len(), body_bytes);
            assert!(!body.contains('\u{1b}'));
            let root = tempfile::tempdir().unwrap();
            let raw_path = root.path().join(name);
            let view_path = save(&raw_path, &raw).unwrap();
            let view = fs::read_to_string(&view_path).unwrap();
            assert!(view.ends_with(&body));
            assert_eq!(fs::read_to_string(&raw_path).unwrap(), raw);
            verified_raw_source(&view_path, &view).unwrap().unwrap();
            println!(
                "{name}: raw={} readable_body={} bytes; both hashes verified",
                raw.len(),
                body.len()
            );
        }
    }
}
