# Full Read Summary

This historical report correctly identified timestamp-derived projection epochs
as a portability risk. Current codec source now derives the default epoch from
the fixed projection kernel, persists it atomically, honors explicit reviewed
overrides, and proves identical defaults across independent runtime
directories. The fixed matrix and smooth entropy gate are covered by exact
regressions; changing an active epoch remains compatibility-sensitive.
