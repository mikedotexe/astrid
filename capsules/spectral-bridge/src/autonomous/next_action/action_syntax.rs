pub(crate) fn strip_action(original: &str, prefix: &str) -> String {
    let upper = original.to_uppercase();
    if upper.starts_with(prefix) {
        // Action text commonly uses `PREFIX: value`; keep only the value.
        original[prefix.len()..]
            .trim_start()
            .trim_start_matches([':', '-', '\u{2014}'])
            .trim()
            .to_string()
    } else {
        String::new()
    }
}
