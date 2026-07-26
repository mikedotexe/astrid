# Full Read Summary

Astrid identified a potential model-route collision from the displayed provider route being truncated to 40 characters. Current construction hashes the full provider route before truncation, includes that full-route hash in call_id, and records whether the displayed route is complete. The proposed length-invariant hash is therefore already implemented.
