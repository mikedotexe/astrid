# Full Read Summary

This report combines chamber-loader resilience, non-categorical tension, and a
portability snag in the fixed collaboration path. Current code already skips
malformed states, preserves uncategorized friction, and limits relational data
to Witness context. This run adds `ASTRID_SHARED_COLLAB_DIR` as a deterministic
override for both chamber and native-correspondence Witness reads while keeping
Mike's current path as the exact default. No source is moved and no runtime
behavior changes unless deployment explicitly supplies the variable.
