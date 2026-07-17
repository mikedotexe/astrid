# Full Read Summary

Minime reports that RUN_PYTHON parsing can become ambiguous when prompts
contain quotes, comments, colons, equals signs, flag-like text, or path-like
phrases. Current parsing is extracted into `minime_autonomy/runtime_actions.py`
and handles explicit flags, quoted and multiline values, code boundaries,
comments, missing terminators, nested punctuation, and script-name
sanitization. The root agent remains an import and CLI facade, and Recess and
Focused policy remain distinct. The dense regression suite verifies the
reported ambiguity without broadening execution authority.
