"""Control-plane error taxonomy and stable CLI exit codes."""


class StewardControlError(RuntimeError):
    """Base class for expected control-plane failures."""

    exit_code = 5


class ConfigurationError(StewardControlError):
    """Configuration or invocation is invalid."""

    exit_code = 2


class PausedError(StewardControlError):
    """The control plane is paused."""

    exit_code = 3


class BusyError(StewardControlError):
    """Another live lease owns the control plane."""

    exit_code = 3


class LeaseError(StewardControlError):
    """A lease token is missing, stale, or does not own the run."""

    exit_code = 3


class EvidenceInvalidError(StewardControlError):
    """Canonical evidence cannot be trusted for a mutating run."""

    exit_code = 4
