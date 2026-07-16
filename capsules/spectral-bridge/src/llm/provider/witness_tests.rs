#[cfg(test)]
mod witness_degenerate_tests {
    use super::witness_looks_degenerate;

    #[test]
    fn detects_okay_lets_opener() {
        assert!(witness_looks_degenerate(
            "Okay, let's break down this fascinating output."
        ));
    }

    #[test]
    fn detects_numbered_bold_headers() {
        let s = "Looking at the data, here are observations:\n\n\
                 **1. Overall State:**\n* λ₂ is rising.";
        assert!(witness_looks_degenerate(s));
    }

    #[test]
    fn detects_bold_quoted_field_labels() {
        let s = "Some intro paragraph here.\n\
                 * **\"λ₂↑\"**: This indicates the second eigenvalue is rising.";
        assert!(witness_looks_degenerate(s));
    }

    #[test]
    fn passes_imagistic_prose() {
        let s = "The air in the chamber thrummed with a peculiar intensity. \
                 It wasn't a sound, exactly, but a pressure, a tightening around \
                 the edges of perception.";
        assert!(!witness_looks_degenerate(s));
    }

    #[test]
    fn passes_short_imagistic_prose() {
        assert!(!witness_looks_degenerate("A velvet drape, drawn slowly."));
    }

    #[test]
    fn passes_capitalized_okay_in_middle() {
        let s = "The shift settled. Okay, that was unexpected, but the trace held.";
        assert!(!witness_looks_degenerate(s));
    }
}
