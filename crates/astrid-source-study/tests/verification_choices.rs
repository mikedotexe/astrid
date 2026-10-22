use astrid_source_study::{Catalog, Command, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const CALLER: &str = "astrid/crates/demo/src/caller.rs";
const HELPER: &str = "astrid/crates/demo/src/helper.rs";

fn setup(files: &[(&str, &str)]) -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    for (source, text) in files {
        let path = temp.path().join(source);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([(
            "astrid".into(),
            temp.path().join("astrid"),
        )]))
        .unwrap(),
        temp.path().join("state"),
    );
    (temp, reader)
}

fn accept(reader: &Reader, output: &StudyOutput) {
    let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
    let response = json!({"message":{"content":"Unresolved.\nNEXT: REST"},"done":true}).to_string();
    reader
        .delivered(&output.page.as_ref().unwrap().id, &request, &response)
        .unwrap();
}

fn state(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&fs::read(temp.path().join("state/reader-v1.json")).unwrap()).unwrap()
}

fn context(output: &StudyOutput) -> &str {
    output
        .text
        .split("Optional ways to check an explanation")
        .next()
        .unwrap()
}

fn session(output: &StudyOutput) -> Option<&str> {
    output
        .text
        .lines()
        .find_map(|line| line.find("SELF_STUDY SESSION ").map(|at| &line[at..]))
}

#[test]
fn help_preserves_distinct_qualified_and_receiver_choices_without_delivering_source() {
    let (temp, reader) = setup(&[(
        CALLER,
        "fn run(left: Client, right: Client) {\n    first::consume();\n    second::consume();\n    left.consume();\n    right.consume();\n    extra();\n    // phantom();\n    let _s = \"fabricated()\";\n}\n",
    )]);
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {CALLER} 1"))
        .unwrap();
    accept(&reader, &page);
    let before = state(&temp);
    let help = reader
        .prepare_action("SELF_STUDY HELP verification")
        .unwrap();
    assert_eq!(help.input_kind, InputKind::Help);
    assert!(help.page.is_none());
    let text = context(&help);
    for expression in [
        "first::consume",
        "second::consume",
        "left.consume",
        "right.consume",
    ] {
        assert!(
            text.contains(&format!("SELF_STUDY RELATE {expression}")),
            "{text}"
        );
    }
    assert!(text.contains("qualification/receiver binding unresolved"));
    assert!(text.contains("not verified call/data flow"));
    assert!(text.contains("not resolved consumers or evidence of deployment"));
    assert!(!text.contains("RELATE consume"));
    assert!(!text.lines().any(|line| line.ends_with("RELATE first")));
    assert!(!text.lines().any(|line| line.ends_with("RELATE left")));
    assert!(!text.contains("extra"));
    assert!(!text.contains("phantom"));
    assert!(!text.contains("fabricated"));
    assert_eq!(text.matches("Source location: SELF_STUDY OPEN").count(), 4);
    assert!(
        !help
            .text
            .contains("RELATE currently accepts a bare identifier")
    );
    for field in [
        "bookmarks",
        "current",
        "progress",
        "pending",
        "pending_session",
        "notebook",
        "questions",
    ] {
        assert_eq!(state(&temp)[field], before[field], "{field}");
    }
    for command in text.lines().filter_map(|line| {
        line.split_once(": SELF_STUDY ")
            .map(|(_, command)| format!("SELF_STUDY {command}"))
    }) {
        assert!(Command::parse(&command).is_ok(), "{command}");
        let selected = reader.prepare_action(&command).unwrap();
        assert_ne!(selected.input_kind, InputKind::Recovery, "{command}");
    }
}

#[test]
fn help_retains_declaration_scope_and_pending_offer_without_acknowledgement() {
    let (temp, reader) = setup(&[(
        CALLER,
        "mod nested {\n    struct Client;\n    impl Client {\n        fn consume(&self) {}\n    }\n}\n",
    )]);
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {CALLER} 1"))
        .unwrap();
    accept(&reader, &page);
    reader
        .prepare_action(&format!("SELF_STUDY OPEN {CALLER} 4"))
        .unwrap();
    let before = state(&temp);
    let help = reader
        .prepare_action("SELF_STUDY HELP verification")
        .unwrap();
    assert!(context(&help).contains("nested::impl Client::consume"));
    assert!(context(&help).contains("SELF_STUDY RELATE nested::Client::consume"));
    assert!(context(&help).contains("A separate source offer is pending"));
    for field in [
        "bookmarks",
        "current",
        "progress",
        "pending",
        "pending_session",
    ] {
        assert_eq!(state(&temp)[field], before[field], "{field}");
    }
}

#[test]
fn relate_offers_distinct_definition_caller_and_condition_locations_only_as_candidates() {
    let (temp, reader) = setup(&[
        (
            CALLER,
            "fn run(client: Client, ready: bool) {\n    if ready {\n        client.consume();\n    }\n}\n",
        ),
        (HELPER, "impl Client {\n    fn consume(&self) {}\n}\n"),
    ]);
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {CALLER} 1"))
        .unwrap();
    accept(&reader, &page);
    let before = state(&temp);
    let result = reader.prepare_action("SELF_STUDY RELATE consume").unwrap();
    let command = session(&result).expect(&result.text);
    assert_eq!(
        Command::parse(command).unwrap(),
        Command::Session {
            targets: vec![(HELPER.into(), 2), (CALLER.into(), 3), (CALLER.into(), 2)]
        }
    );
    assert!(result.text.contains("Callee syntax: client.consume"));
    assert!(
        result
            .text
            .contains("Definition syntax: impl Client::consume")
    );
    assert!(result.text.contains("ambiguity unresolved"));
    assert!(result.text.contains("not verified call/data flow"));
    assert!(
        result
            .text
            .contains("third location is enclosing branch/loop syntax")
    );
    assert!(
        result
            .text
            .contains("moves the reading position to its last page")
    );
    for field in [
        "bookmarks",
        "current",
        "progress",
        "pending",
        "pending_session",
    ] {
        assert_eq!(state(&temp)[field], before[field], "{field}");
    }
    let selected = reader.prepare_action(command).unwrap();
    assert_eq!(selected.input_kind, InputKind::SourceSession);
    assert!(selected.text.contains("if ready"));
}

#[test]
fn multiple_definitions_stay_unresolved_and_show_qualified_contexts() {
    let (_temp, reader) = setup(&[(
        CALLER,
        "mod first { fn consume() {} }\nmod second { fn consume() {} }\nfn run() { first::consume(); second::consume(); }\n",
    )]);
    let result = reader.prepare_action("SELF_STUDY RELATE consume").unwrap();
    assert!(
        result
            .text
            .contains("Multiple parsed definition candidates; ambiguity unresolved")
    );
    assert!(result.text.contains("declaration first::consume"));
    assert!(result.text.contains("declaration second::consume"));
    assert!(result.text.contains("callee first::consume"));
    assert!(result.text.contains("callee second::consume"));
    assert!(session(&result).is_none());
}

#[test]
fn a_single_location_or_test_only_caller_does_not_make_a_comparison() {
    for text in [
        "fn consume() { consume(); }\n",
        "fn consume() {}\n#[test]\nfn test_call() { consume(); }\n",
        "fn consume() {}\n// consume();\nconst TEXT: &str = \"consume()\";\n",
    ] {
        let (_temp, reader) = setup(&[(CALLER, text)]);
        let result = reader.prepare_action("SELF_STUDY RELATE consume").unwrap();
        assert!(session(&result).is_none(), "{}", result.text);
    }
}

#[test]
fn python_receiver_context_is_preserved_and_multiline_callees_are_not_flattened_into_commands() {
    let source = "astrid/crates/demo/src/caller.py";
    let (_temp, reader) = setup(&[(
        source,
        "def consume():\n    pass\n\ndef run(client, ready):\n    if ready:\n        client.consume()\n        (client\n            .consume)()\n",
    )]);
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {source} 1"))
        .unwrap();
    accept(&reader, &page);
    let help = reader
        .prepare_action("SELF_STUDY HELP verification")
        .unwrap();
    assert!(context(&help).contains("SELF_STUDY RELATE client.consume"));
    assert!(!context(&help).contains("SELF_STUDY FIND (client"));
    let result = reader.prepare_action("SELF_STUDY RELATE consume").unwrap();
    assert!(result.text.contains("callee client.consume"));
    assert!(session(&result).is_some());
}

#[test]
fn syntax_failures_and_changed_revisions_never_supply_derived_locations() {
    for text in ["fn broken( { consume();", "fn valid() {}\n"] {
        let (temp, reader) = setup(&[(CALLER, text)]);
        let page = reader
            .prepare_action(&format!("SELF_STUDY OPEN {CALLER} 1"))
            .unwrap();
        accept(&reader, &page);
        if text.contains("valid") {
            fs::write(temp.path().join(CALLER), "fn changed() { consume(); }").unwrap();
        }
        let help = reader
            .prepare_action("SELF_STUDY HELP verification")
            .unwrap();
        assert!(!context(&help).contains("Source location:"));
        assert!(!context(&help).contains("Lookup candidate:"));
    }
}

#[test]
fn a_bounded_scan_does_not_choose_a_comparison_from_partial_candidates() {
    let text = format!(
        "fn consume() {{}}\nfn run() {{\n{}\n}}\n",
        "consume();\n".repeat(1_501)
    );
    let (_temp, reader) = setup(&[(CALLER, &text)]);
    let result = reader.prepare_action("SELF_STUDY RELATE consume").unwrap();
    assert!(result.text.contains("scan limit reached: true"));
    assert!(session(&result).is_none());
}

#[test]
fn long_contexts_keep_help_bounded_and_location_commands_whole() {
    let source = format!("astrid/crates/demo/src/{}caller.rs", "nested/".repeat(60));
    let receiver = "receiver".repeat(25);
    let text = format!(
        "fn run() {{\n    {receiver}.first();\n    {receiver}.second();\n    {receiver}.third();\n    {receiver}.fourth();\n}}\n"
    );
    let (_temp, reader) = setup(&[(&source, &text)]);
    let page = reader
        .prepare_action(&format!("SELF_STUDY OPEN {source} 1"))
        .unwrap();
    accept(&reader, &page);
    let help = reader
        .prepare_action("SELF_STUDY HELP verification")
        .unwrap();
    assert!(help.text.len() <= astrid_source_study::MAX_PAGE_BYTES);
    assert!(context(&help).contains("Additional choices omitted by display bound"));
    assert!(!context(&help).contains("Lookup candidate:"));
    let commands = context(&help)
        .lines()
        .filter_map(|line| line.strip_prefix("Source location: "))
        .collect::<Vec<_>>();
    assert!(!commands.is_empty());
    for command in commands {
        assert!(
            matches!(Command::parse(command), Ok(Command::Open { source: actual, line }) if actual == source && (2..=5).contains(&line))
        );
    }
}

#[test]
fn qualified_rust_constructor_never_collapses_to_an_unrelated_terminal_name() {
    // Baseline 68369bfa0a5e8a25b03dd7a70aa96b0782c35762 rejected ':' and '.'
    // in relationships::valid_symbol, contrary to the approved plan's premise.
    let unrelated = "astrid/crates/demo/src/restart.rs";
    let (_temp, reader) = setup(&[
        (HELPER, "impl EventBus {\n    fn new() {}\n}\n"),
        (CALLER, "fn run() {\n    EventBus::new();\n}\n"),
        (
            unrelated,
            "impl RestartTracker {\n    fn new() {}\n}\nfn restart() { RestartTracker::new(); }\n",
        ),
    ]);
    let result = reader
        .prepare_action("SELF_STUDY RELATE EventBus::new")
        .unwrap();
    assert_eq!(result.input_kind, InputKind::Relationships);
    assert!(result.text.contains("Definition syntax: EventBus::new"));
    assert!(result.text.contains("Callee syntax: EventBus::new"));
    assert!(!result.text.contains(unrelated));
    assert!(!result.text.contains("RestartTracker"));
    assert!(result.text.contains("No bare-name fallback"));
    assert_eq!(
        Command::parse(session(&result).expect(&result.text)).unwrap(),
        Command::Session {
            targets: vec![(HELPER.into(), 2), (CALLER.into(), 2)]
        }
    );
    let bare = reader.prepare_action("SELF_STUDY RELATE new").unwrap();
    assert!(bare.text.contains("RestartTracker"));
    assert!(bare.text.contains("Multiple parsed definition candidates"));
    assert!(session(&bare).is_none());
}

#[test]
fn exact_qualified_syntax_ranks_before_suffixes_without_dropping_prefixes() {
    let exact = "astrid/crates/demo/src/z_exact.rs";
    let suffix = "astrid/crates/demo/src/a_suffix.rs";
    let (_temp, reader) = setup(&[
        (
            exact,
            "impl EventBus { fn new() {} }\nfn run() { EventBus::new(); }\n",
        ),
        (
            suffix,
            "mod nested { impl EventBus { fn new() {} } }\nfn run() { nested::EventBus::new(); }\n",
        ),
        (
            HELPER,
            "fn unrelated() { NotEventBus::new(); EventBus::newer(); }\n",
        ),
    ]);
    let result = reader
        .prepare_action("SELF_STUDY RELATE EventBus::new")
        .unwrap();
    let text = &result.text;
    assert!(
        text.find(&format!("SELF_STUDY OPEN {exact} 2")).unwrap()
            < text.find(&format!("SELF_STUDY OPEN {suffix} 2")).unwrap()
    );
    assert!(text.contains("callee nested::EventBus::new"));
    assert!(!text.contains(HELPER));
    assert!(session(&result).is_none());
    let selected = reader
        .prepare_action("SELF_STUDY RELATE nested::EventBus::new")
        .unwrap();
    assert!(!selected.text.contains(exact));
    assert!(selected.text.contains(suffix));
}

#[test]
fn python_paths_preserve_receiver_spelling_without_inferring_receiver_types() {
    let source = "astrid/crates/demo/src/client.py";
    let (_temp, reader) = setup(&[(
        source,
        "class Client:\n    def publish(self):\n        pass\n\nclass Other:\n    def publish(self):\n        pass\n\ndef run(client):\n    client.publish()\n    Client.publish()\n    other.publish()\n",
    )]);
    let receiver = reader
        .prepare_action("SELF_STUDY RELATE client.publish")
        .unwrap();
    assert!(receiver.text.contains("callee client.publish"));
    assert!(
        receiver
            .text
            .contains("receiver/type and Self bindings remain unresolved")
    );
    assert!(!receiver.text.contains("declaration Client.publish"));
    assert!(!receiver.text.contains("other.publish()"));
    assert!(session(&receiver).is_none());
    let class = reader
        .prepare_action("SELF_STUDY RELATE Client.publish")
        .unwrap();
    assert!(class.text.contains("declaration Client.publish"));
    assert!(class.text.contains("callee Client.publish"));
    assert!(!class.text.contains("declaration Other.publish"));
    assert!(!class.text.contains("callee client.publish"));
    assert!(session(&class).is_some());
}

#[test]
fn self_and_unknown_receivers_are_not_substituted_with_a_matching_method() {
    let (_temp, reader) = setup(&[
        (
            HELPER,
            "impl EventBus { fn new() {} fn publish(&self) {} }\n",
        ),
        (
            CALLER,
            "impl Unknown {\n    fn run(&self) {\n        Self::new();\n        self.publish();\n    }\n}\n",
        ),
    ]);
    for query in ["Self::new", "self.publish"] {
        let result = reader
            .prepare_action(&format!("SELF_STUDY RELATE {query}"))
            .unwrap();
        assert_eq!(result.input_kind, InputKind::Relationships);
        assert!(result.text.contains(&format!("callee {query}")));
        assert!(!result.text.contains(HELPER));
        assert!(result.text.contains("Self bindings remain unresolved"));
        assert!(session(&result).is_none());
    }
}

#[test]
fn qualified_queries_keep_pagination_and_do_not_add_bare_spelling_hints() {
    let history = "astrid/docs/steward-notes/review.md";
    let text = "Earlier EventBus::new account; execution unknown.\n".repeat(100);
    let (_temp, reader) = setup(&[
        (history, &text),
        (CALLER, "fn run() { EventBus::new(); }\n"),
        (HELPER, "fn new() {}\n"),
    ]);
    let first = reader
        .prepare_action("SELF_STUDY RELATE EventBus::new")
        .unwrap();
    assert!(
        first
            .text
            .contains("Next: SELF_STUDY RELATE EventBus::new --page 2")
    );
    let second = reader
        .prepare_action("SELF_STUDY RELATE EventBus::new --page 2")
        .unwrap();
    assert_eq!(second.input_kind, InputKind::Relationships);
    assert!(second.text.contains(history));
    for output in [&first, &second] {
        assert!(
            output
                .text
                .contains("Historical commentary: 100 matching lines")
        );
        assert!(!output.text.contains("Candidate identifier text"));
        assert!(!output.text.contains(HELPER));
        assert!(output.text.len() <= astrid_source_study::MAX_PAGE_BYTES);
    }
}

#[test]
fn qualified_identifier_grammar_is_bounded_and_rejects_ambiguous_syntax() {
    let (_temp, reader) = setup(&[(CALLER, "fn run() {}\n")]);
    for query in ["a", "_a2", "a::b::c", "client.store.publish", "Self::new"] {
        assert_eq!(
            reader
                .prepare_action(&format!("SELF_STUDY RELATE {query}"))
                .unwrap()
                .input_kind,
            InputKind::Relationships,
            "{query}"
        );
    }
    for query in [
        "::new",
        "a::",
        "a:::new",
        "a:new",
        "a..new",
        ".new",
        "a.",
        "a::b.new",
        "a.2new",
        "a::new()",
        "a::<T>::new",
        "a new",
        "a|new",
    ] {
        assert_eq!(
            reader
                .prepare_action(&format!("SELF_STUDY RELATE {query}"))
                .unwrap()
                .input_kind,
            InputKind::Recovery,
            "{query}"
        );
    }
    let longest = format!("a::{}", "b".repeat(157));
    assert_eq!(
        reader
            .prepare_action(&format!("SELF_STUDY RELATE {longest}"))
            .unwrap()
            .input_kind,
        InputKind::Relationships
    );
    assert_eq!(
        reader
            .prepare_action(&format!("SELF_STUDY RELATE {longest}b"))
            .unwrap()
            .input_kind,
        InputKind::Recovery
    );
}
