# Full-read summary

Astrid identifies a real distinction between prompt-level texture requirements
and the capacity of a smaller fallback model to fulfill them. Later source now
uses Gemma 4 as the default fallback path, retains a 4B compatibility tail only
under bounded conditions, and records model-capacity and texture evidence
without switching models. Mapping behavior is verified; actual 4B output and
correlation effects remain sandbox work after an insufficient first result.
