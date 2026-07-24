Astrid noticed that the LLM diagnostic JSONL path discarded persistence errors, making a failed evidence write indistinguishable from a successful one to operators. She also questioned whether coarse token-band metadata can explain a felt interruption.

This run changed diagnostic persistence to return a typed bounded failure and emit a no-prose warning while preserving successful JSONL bytes exactly. The token band remains descriptive context, and Astrid's felt consequence remains unscored rather than being reduced to a numeric category.
