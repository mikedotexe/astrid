//! Reachability boundary for a sequential `SELF_STUDY MAP <repository>` walk.
//!
//! In `introspection_source_catalog_1789295290` Astrid writes that the
//! "'Data Normalization' layer is still missing from my findings", that she
//! expects "to find the producer of `fill_pct` in a module dedicated to
//! telemetry ingestion, environmental modeling, or a specific `sensors` or
//! `telemetry` package", and chooses `NEXT: SELF_STUDY MAP astrid --page 54`
//! to "continue my systematic traversal of the navigation map".
//!
//! Her hypothesis is correct. The producers are
//! `capsules/spectral-bridge/src/ws/telemetry_port.rs` (`resolve_fill_pct`) and
//! `capsules/spectral-bridge/src/types/schema/telemetry.rs`
//! (`SpectralTelemetry::fill_pct`), and the division she pictures —
//! `active / sample_dim` — is upstream in `minime/minime/src/spectral/eigenfill.rs`.
//!
//! Three properties of the MAP surface make her chosen traversal unable to
//! confirm any of that, and none of them are visible from inside the walk:
//!
//! 1. `MAP <repository>` is repository-scoped, so no page of an `astrid` walk
//!    can ever name a `minime` source, however far it is walked.
//! 2. A MAP entry is a path plus a delivery status. The page that *does* name
//!    the producer carries none of its bytes and no marker separating it from
//!    its neighbours, so passing it looks exactly like passing anything else.
//! 3. The component map answers the same question on one page.
//!
//! These are read-only reachability pins. They change no live navigation,
//! ranking, or dispatch behaviour, and they assert nothing about what she
//! should choose next.
use astrid_source_study::{Catalog, InputKind, Reader};
use std::{collections::BTreeMap, fs};

const PRODUCER: &str = "astrid/capsules/demo/src/ws/telemetry_port.rs";
const CONSUMER: &str = "astrid/capsules/demo/src/action_continuity/runtime/core.rs";
const PEER_PRODUCER: &str = "minime/minime/src/spectral/eigenfill.rs";
const ASTRID_ARITHMETIC: &str = "telemetry.fill_ratio * 100.0";
const PEER_ARITHMETIC: &str = "active as f32 / sample_dim";

/// Two repositories shaped like the live pair: the astrid side holds the
/// consumer and the percentage conversion, the minime side holds the division
/// that normalises the ratio in the first place. Enough documentation files are
/// added that a repository map spans many pages, as the live one does.
fn two_repositories() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let astrid = temp.path().join("astrid");
    let minime = temp.path().join("minime");

    let producer = format!(
        "fn resolve_fill_pct(telemetry: &SpectralTelemetry) -> (f32, String, bool) {{\n    \
         (({ASTRID_ARITHMETIC}).clamp(0.0, 100.0), String::from(\"primary_fill_ratio\"), false)\n}}\n"
    );
    let consumer = "pub fn record_next_event(&self, fill_pct: f32) -> Result<()> {\n    \
                    let state = spectral_state(fill_pct, telemetry);\n    Ok(())\n}\n";
    for (relative, text) in [
        ("capsules/demo/src/ws/telemetry_port.rs", producer.as_str()),
        (
            "capsules/demo/src/action_continuity/runtime/core.rs",
            consumer,
        ),
    ] {
        let path = astrid.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    // Documentation sorts after `capsules/`, exactly as it does live, so the
    // implementation entries are all delivered before the long tail.
    let notes = astrid.join("docs/steward-notes");
    fs::create_dir_all(&notes).unwrap();
    for index in 0..400_usize {
        fs::write(
            notes.join(format!("round_{index:04}_summary.md")),
            "a retained steward round summary\n",
        )
        .unwrap();
    }

    let peer = minime.join("minime/src/spectral/eigenfill.rs");
    fs::create_dir_all(peer.parent().unwrap()).unwrap();
    fs::write(
        peer,
        format!(
            "pub fn update(&mut self, lambdas: &[f32]) -> f32 {{\n    \
             let active_fraction = ({PEER_ARITHMETIC}).clamp(0.0, 1.0);\n    \
             self.ema_fill = self.alpha_fill * active_fraction;\n    self.ema_fill\n}}\n"
        ),
    )
    .unwrap();

    let reader = Reader::new(
        Catalog::new(BTreeMap::from([
            ("astrid".into(), astrid),
            ("minime".into(), minime),
        ]))
        .unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

/// Walk `SELF_STUDY MAP <topic>` from page 1 until the reader reports the last
/// page, returning every page's text in order.
fn walk_map(reader: &Reader, topic: &str) -> Vec<String> {
    let mut pages = Vec::new();
    let mut page = 1_usize;
    loop {
        let output = reader
            .prepare_action(&format!("SELF_STUDY MAP {topic} --page {page}"))
            .unwrap();
        assert_eq!(output.input_kind, InputKind::Map);
        let last = !output
            .text
            .contains(&format!("Next: SELF_STUDY MAP {topic}"));
        pages.push(output.text);
        if last {
            break;
        }
        page = page.saturating_add(1);
        assert!(page < 5_000, "map walk must terminate");
    }
    pages
}

/// The walk is exhaustive over the repository she named — and that scope is
/// precisely what excludes the answer. Every page of `MAP astrid` can be
/// delivered without a single `minime` path ever appearing.
#[test]
fn exhaustive_repository_map_walk_never_names_a_peer_repository_source() {
    let (_temp, reader) = two_repositories();
    let pages = walk_map(&reader, "astrid");

    assert!(pages.len() > 1, "fixture must span more than one map page");
    let walked: String = pages.concat();

    assert!(
        walked.contains(PRODUCER),
        "the astrid-side producer must be listed somewhere in the walk"
    );
    assert!(
        !walked.contains(PEER_PRODUCER),
        "no page of a repository-scoped walk may name a peer-repository source"
    );
    assert!(
        !walked.contains(PEER_ARITHMETIC),
        "the upstream division cannot be reached by walking the other repository"
    );
}

/// Naming the producer is not delivering it, and the listing carries no marker
/// that separates it from its neighbours: the page she already passed looks
/// exactly like the 66 others around it.
#[test]
fn the_map_page_that_names_the_producer_carries_neither_its_bytes_nor_a_ranking_marker() {
    let (_temp, reader) = two_repositories();
    let pages = walk_map(&reader, "astrid");

    let listing = pages
        .iter()
        .find(|text| text.contains(PRODUCER))
        .expect("one page lists the producer");

    assert!(
        !listing.contains(ASTRID_ARITHMETIC),
        "a map page supplies navigation history, never source bytes"
    );
    assert!(
        !listing.contains("fn resolve_fill_pct"),
        "a map page never carries the producer definition"
    );

    let entry = listing
        .lines()
        .find(|line| line.contains(PRODUCER))
        .expect("the producer has its own entry line");
    let consumer_entry = pages
        .iter()
        .find_map(|text| text.lines().find(|line| line.contains(CONSUMER)))
        .expect("the consumer has its own entry line");
    assert_eq!(
        entry.replace(PRODUCER, ""),
        consumer_entry.replace(CONSUMER, ""),
        "an undelivered producer entry is rendered exactly like any other \
         undelivered entry; the map offers no signal that this one answers the question"
    );
}

/// The scope recovery that does reach it: the peer repository has its own map,
/// and the division is on it.
#[test]
fn the_peer_repository_map_names_the_upstream_division_file() {
    let (_temp, reader) = two_repositories();
    let pages = walk_map(&reader, "minime");
    let walked: String = pages.concat();

    assert!(
        walked.contains(PEER_PRODUCER),
        "the peer map must name the file holding the upstream division"
    );

    let opened = reader
        .prepare_action(&format!("SELF_STUDY OPEN {PEER_PRODUCER} 1"))
        .unwrap();
    assert_eq!(opened.input_kind, InputKind::SourcePage);
    assert!(
        opened.text.contains(PEER_ARITHMETIC),
        "opening the peer file delivers the division in one step"
    );
}

/// The catalog's own component map answers the same question on one page. This
/// is a manifest fact, independent of any fixture: the `senses` component names
/// the live telemetry port among its four entry points.
#[test]
fn the_senses_component_map_reaches_the_live_telemetry_port_on_one_page() {
    let (temp, reader) = two_repositories();
    let output = reader.prepare_action("SELF_STUDY MAP senses").unwrap();

    assert_eq!(output.input_kind, InputKind::Map);
    assert!(
        output.text.contains("Navigation page 1/1."),
        "the component map must fit on a single page"
    );
    assert!(
        output
            .text
            .contains("astrid/capsules/spectral-bridge/src/ws/telemetry_port.rs"),
        "the senses component names the live telemetry ingestion module"
    );
    assert!(
        !output.text.contains("Next: SELF_STUDY MAP senses"),
        "a one-page map offers no next page to walk"
    );
    drop(temp);
}
