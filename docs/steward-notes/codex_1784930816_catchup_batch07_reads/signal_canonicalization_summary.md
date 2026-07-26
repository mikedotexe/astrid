# Full Read Summary

The historical Signal Spine types report correctly asked for bounded recursion and deterministic measurement identity, but JSON `Value` is an owned tree and cannot contain reference cycles. Current validation rejects measurement nesting beyond 32 levels before canonical hashing, sorts object keys deterministically, and preserves non-integer measurement lexemes as strings by contract. These identities establish evidence integrity, not personal identity or felt continuity.
