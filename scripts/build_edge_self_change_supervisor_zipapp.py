#!/usr/bin/env python3
"""Build the immutable self-change supervisor as a deterministic Python zipapp."""

from __future__ import annotations

import argparse
import hashlib
import os
import stat
import tempfile
import zipfile
from pathlib import Path

ENTRY = b"from edge_self_change.cli import main\nraise SystemExit(main())\n"
SHEBANG = b"#!/usr/bin/python3\n"


class BuildError(RuntimeError):
    """Invalid source or output boundary."""


def stable_source(path: Path) -> bytes:
    before = path.lstat()
    if path.is_symlink() or not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise BuildError(f"supervisor source is not a regular unlinked file: {path}")
    data = path.read_bytes()
    after = path.lstat()
    if (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
    ) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns) or len(data) != before.st_size:
        raise BuildError(f"supervisor source changed while read: {path}")
    return data


def zip_info(name: str) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    info.create_system = 3
    info.external_attr = 0o100644 << 16
    return info


def build(source: Path, output: Path) -> dict[str, object]:
    if source.is_symlink() or not source.is_dir():
        raise BuildError("supervisor package root must be a non-symlink directory")
    files = sorted(source.glob("*.py"))
    if not files or {path.name for path in files} < {"__init__.py", "cli.py", "model.py", "profiles.py", "supervisor.py"}:
        raise BuildError("supervisor package is incomplete")
    if any(path.name.startswith(".") for path in files):
        raise BuildError("hidden supervisor source is refused")
    if output.exists() or output.is_symlink() or not output.parent.is_dir():
        raise BuildError("refusing to overwrite output or use an absent output parent")
    temporary: str | None = None
    try:
        descriptor, temporary = tempfile.mkstemp(prefix=".edge-supervisor-", dir=output.parent)
        with os.fdopen(descriptor, "wb") as raw:
            raw.write(SHEBANG)
            with zipfile.ZipFile(raw, "w") as archive:
                archive.writestr(zip_info("__main__.py"), ENTRY)
                for path in files:
                    archive.writestr(zip_info(f"edge_self_change/{path.name}"), stable_source(path))
            raw.flush()
            os.fsync(raw.fileno())
        os.chmod(temporary, 0o555)
        os.link(temporary, output)
        os.unlink(temporary)
        temporary = None
    finally:
        if temporary is not None:
            Path(temporary).unlink(missing_ok=True)
    data = output.read_bytes()
    return {
        "output": str(output),
        "sha256": hashlib.sha256(data).hexdigest(),
        "source_files": len(files),
        "bytes": len(data),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path(__file__).with_name("edge_self_change"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = build(args.source, args.output)
    except (BuildError, OSError, zipfile.BadZipFile) as error:
        parser.error(str(error))
    print(" ".join(f"{key}={value}" for key, value in result.items()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
