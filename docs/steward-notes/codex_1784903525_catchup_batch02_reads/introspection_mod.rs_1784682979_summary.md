# Full Read Summary

Astrid worried that peer scalar parsing ran synchronously in witness capture. Current implementation refreshes the bounded three-field peer cache on a separate 500 ms thread and reads the cached snapshot below one millisecond p95. Missing and malformed states are explicit, so the historical latency concern is addressed.
