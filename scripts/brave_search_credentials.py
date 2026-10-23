#!/usr/bin/env python3
"""Operator-only, offline installation of a Brave Search credential.

Use ``install --profile /absolute/profile``; no profile is selected implicitly.
The default destination is the account's ~/.config/astrid/credentials/
brave-search-api-key (HOME and XDG environment overrides are not consulted).
``--restrict-profile-permissions`` explicitly authorizes fchmod of the verified
profile inode to 0600, never a content rewrite. No shell or network is invoked.

The deliberately narrow profile grammar accepts one physical line containing
NAME=token or export NAME=token, with optional single/double literal quotes and
an optional whitespace-separated comment. Other references to NAME, multiline
quotes, continuations and here-documents are refused, not interpreted. Other
single-line profile commands are lexed only, never executed. This is extraction
of an explicit literal, not evaluation of the profile's shell semantics.

For an ordinary profile with unrelated multiline shell code, an operator may
select ``--assignment-line N`` (one-based). That mode validates only the selected
physical line as a literal assignment. It ignores unrelated shell syntax, but
refuses any other direct reference to NAME, a preceding line continuation, and
any ``<<`` marker outside whole-line comments (including quoted markers). This
conservative heredoc exclusion prevents data lines masquerading as assignments
without attempting a full shell parser. Backslash-newline folding is used only
to detect hidden references/heredoc markers, never to compute a credential.
This is explicitly selected literal extraction, NOT sourced/environment lookup.

Public APIs: install_credentials(profile, destination=None, ...), and
inspect_credentials(destination=None). Results and exceptions contain no path,
content, length or digest. Inspection checks metadata only, not token validity.
Callers must not log inputs, frame locals, or private parser return values.

POSIX only. Every path component is opened without following symlinks. Ancestors
must be root/account-owned and not group/world writable; immediate parents must
be account-owned, and the destination parent must be 0700. Newly created
directories are 0700. Access-granting ACLs are refused; macOS deny-only ACLs
are allowed. macOS ACLs are checked through its native API.
The account itself and root are trusted not to race operations maliciously.
"""

from __future__ import annotations

import argparse
import contextlib
import ctypes
import errno
import functools
import hmac
import json
import os
from pathlib import Path
import pwd
import re
import secrets
import shlex
import stat
import sys
from typing import Iterator


NAME = "BRAVE_SEARCH_API_KEY"
MAX_PROFILE_BYTES = 1024 * 1024
MAX_TOKEN_BYTES = 4096
_TOKEN = r"[A-Za-z0-9._~+/=-]{1,4096}"
_ASSIGNMENT = re.compile(
    rf"[ \t]*(?:export[ \t]+)?{NAME}="
    rf"(?:{_TOKEN}|'({_TOKEN})'|\"({_TOKEN})\")"
    r"(?:[ \t]+(?:\#.*)?)?"
)
_REFERENCE = re.compile(rf"(?<![A-Za-z0-9_]){NAME}(?![A-Za-z0-9_])")
_DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
_FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


class CredentialError(Exception):
    """A fixed, content-free operator error."""


def _redacted_errors(function):
    @functools.wraps(function)
    def guarded(*args, **kwargs):
        try:
            return function(*args, **kwargs)
        except CredentialError:
            raise
        except Exception:
            # OS/decoder/parser exceptions can contain paths or source bytes.
            raise CredentialError("credential operation failed") from None
    return guarded


def _uid() -> int:
    if os.getuid() != os.geteuid():
        raise CredentialError("privilege-changing invocation is not supported")
    return os.getuid()


@_redacted_errors
def default_destination() -> Path:
    """Return the account-based default, without reading any credential."""
    return Path(pwd.getpwuid(_uid()).pw_dir) / ".config/astrid/credentials/brave-search-api-key"


def _path_parts(path: str | os.PathLike[str]) -> tuple[str, ...]:
    raw = os.fspath(path)
    if not isinstance(raw, str) or not re.fullmatch(r"/[A-Za-z0-9_. /-]+", raw):
        raise CredentialError("an absolute literal path is required")
    parts = tuple(raw.split("/")[1:])
    if any(not part or part in {".", "..", ".git", ".hg", ".svn"}
           or part.startswith("-") or part != part.strip() for part in parts):
        raise CredentialError("unsafe path component")
    return parts


def _no_acl(fd: int) -> None:
    if sys.platform == "darwin":
        # Darwin does not expose extended ACLs through listxattr. acl_get_fd_np
        # returns opaque entries; inspect only their allow/deny tags.
        libc = ctypes.CDLL(None, use_errno=True)
        libc.acl_get_fd_np.argtypes = [ctypes.c_int, ctypes.c_int]
        libc.acl_get_fd_np.restype = ctypes.c_void_p
        libc.acl_get_entry.argtypes = [ctypes.c_void_p, ctypes.c_int,
                                      ctypes.POINTER(ctypes.c_void_p)]
        libc.acl_get_entry.restype = ctypes.c_int
        libc.acl_get_tag_type.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int)]
        libc.acl_get_tag_type.restype = ctypes.c_int
        libc.acl_free.argtypes = [ctypes.c_void_p]
        libc.acl_free.restype = ctypes.c_int
        acl = libc.acl_get_fd_np(fd, 0x00000100)  # ACL_TYPE_EXTENDED
        if not acl:
            # An open inode without an extended ACL reports ENOENT on Darwin.
            if ctypes.get_errno() == errno.ENOENT:
                return
            raise CredentialError("unable to verify filesystem permissions")
        try:
            entry = ctypes.c_void_p()
            entry_id = 0  # ACL_FIRST_ENTRY
            while True:
                ctypes.set_errno(0)
                result = libc.acl_get_entry(acl, entry_id, ctypes.byref(entry))
                if result != 0:
                    if ctypes.get_errno() != errno.EINVAL:
                        raise CredentialError("unable to verify filesystem permissions")
                    break
                tag = ctypes.c_int()
                if libc.acl_get_tag_type(entry, ctypes.byref(tag)) != 0:
                    raise CredentialError("unable to verify filesystem permissions")
                if tag.value != 2:  # ACL_EXTENDED_DENY
                    raise CredentialError("access-granting ACLs are not supported")
                entry_id = -1  # ACL_NEXT_ENTRY
        finally:
            libc.acl_free(acl)
    elif sys.platform.startswith("linux"):
        if any(name in {"system.posix_acl_access", "system.posix_acl_default"}
               for name in os.listxattr(fd)):
            raise CredentialError("extended filesystem permissions are not supported")
    else:
        raise CredentialError("unsupported filesystem permission platform")


def _check_directory(fd: int, *, immediate: bool = False, private: bool = False) -> None:
    info = os.fstat(fd)
    uid = _uid()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid not in {0, uid}:
        raise CredentialError("directory ownership is unsafe")
    if immediate and info.st_uid != uid:
        raise CredentialError("parent directory must be account-owned")
    mode = stat.S_IMODE(info.st_mode)
    if mode & 0o7022 or (private and mode != 0o700):
        raise CredentialError("directory permissions are unsafe")
    _no_acl(fd)


def _reject_workspace(fd: int) -> None:
    # .git may be a directory, worktree pointer, or symlink; any is disallowed.
    for marker in (".git", ".hg", ".svn", "AGENTS.md"):
        try:
            os.stat(marker, dir_fd=fd, follow_symlinks=False)
        except FileNotFoundError:
            continue
        raise CredentialError("credential destination must be outside workspaces")


@contextlib.contextmanager
def _parent(path: str | os.PathLike[str], *, destination: bool,
            create: bool = False) -> Iterator[tuple[int, str]]:
    parts = _path_parts(path)
    with contextlib.ExitStack() as stack:
        fd = os.open("/", _DIR_FLAGS)
        stack.callback(os.close, fd)
        _check_directory(fd)
        if destination:
            _reject_workspace(fd)
        for part in parts[:-1]:
            try:
                child = os.open(part, _DIR_FLAGS, dir_fd=fd)
            except FileNotFoundError:
                if not create:
                    raise
                # Never create children directly in a foreign-owned ancestor.
                _check_directory(fd, immediate=True)
                try:
                    os.mkdir(part, mode=0o700, dir_fd=fd)
                except FileExistsError:
                    pass
                else:
                    os.fsync(fd)
                child = os.open(part, _DIR_FLAGS, dir_fd=fd)
            stack.callback(os.close, child)
            _check_directory(child)
            if destination:
                _reject_workspace(child)
            fd = child
        _check_directory(fd, immediate=True, private=destination)
        yield fd, parts[-1]


def _check_file(info: os.stat_result, *, profile: bool = False,
                restrict: bool = False) -> None:
    if (not stat.S_ISREG(info.st_mode) or info.st_uid != _uid()
            or info.st_nlink != 1):
        raise CredentialError("file type, ownership or link count is unsafe")
    mode = stat.S_IMODE(info.st_mode)
    if mode == 0o600:
        return
    if profile and restrict and mode in {0o400, 0o440, 0o444, 0o600, 0o640, 0o644, 0o604}:
        return
    if profile:
        raise CredentialError("profile must be 0600; explicit permission restriction may be required")
    raise CredentialError("credential file must be 0600")


def _identity(info: os.stat_result) -> tuple[int, int, int, int, int, int, int, int]:
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _verify_name(fd: int, parent: int, name: str) -> None:
    if _identity(os.fstat(fd)) != _identity(os.stat(name, dir_fd=parent, follow_symlinks=False)):
        raise CredentialError("file changed during operation")


def _read_bounded(fd: int, maximum: int) -> bytes:
    chunks = []
    remaining = maximum + 1
    while remaining:
        chunk = os.read(fd, min(remaining, 65536))
        if not chunk:
            break
        chunks.append(chunk)
        remaining -= len(chunk)
    result = b"".join(chunks)
    if len(result) > maximum:
        raise CredentialError("file exceeds supported size")
    return result


def _selected_literal(profile: str, assignment_line: int) -> bytes:
    lines = profile.split("\n")
    if (type(assignment_line) is not int or assignment_line < 1
            or assignment_line > len(lines)):
        raise CredentialError("assignment line must select an existing physical line")
    index = assignment_line - 1
    selected = lines[index]
    if not _ASSIGNMENT.fullmatch(selected):
        raise CredentialError("selected line must be a single literal assignment")
    if index and lines[index - 1].rstrip().endswith("\\"):
        raise CredentialError("selected line must not continue another line")
    # Do not parse unrelated shell syntax or infer whether a shell would execute
    # this line. Scan only for ambiguity, with deliberately conservative rules.
    remaining = "\n".join(line for number, line in enumerate(lines) if number != index)
    folded = remaining.replace("\\\n", "")
    for line in folded.split("\n"):
        if line.lstrip().startswith("#"):
            continue
        if "<<" in line:
            raise CredentialError("selected-line extraction does not support heredoc markers")
        if _REFERENCE.search(line):
            raise CredentialError("another credential reference makes extraction ambiguous")
    tokens = shlex.split(selected, comments=True, posix=True)
    assignment = tokens[1] if tokens[0] == "export" else tokens[0]
    return assignment.removeprefix(NAME + "=").encode("ascii") + b"\n"


def _parse_profile(content: bytes, assignment_line: int | None = None) -> bytes:
    try:
        profile = content.decode("utf-8")
        if "\x00" in profile or "\r" in profile:
            raise CredentialError("unsupported profile syntax")
        if assignment_line is not None:
            return _selected_literal(profile, assignment_line)
        assignments = []
        for line in profile.split("\n"):
            lexer = shlex.shlex(line, posix=True, punctuation_chars=True)
            lexer.whitespace_split = True
            tokens = list(lexer)
            if any("<<" in token for token in tokens) or line.rstrip().endswith("\\"):
                raise CredentialError("multiline shell forms are not supported")
            if not any(_REFERENCE.search(token) for token in tokens):
                continue
            if not _ASSIGNMENT.fullmatch(line):
                raise CredentialError("credential must be a single literal assignment")
            assignment = tokens[1] if tokens[0] == "export" else tokens[0]
            assignments.append(assignment.removeprefix(NAME + "=").encode("ascii"))
        if len(assignments) != 1:
            raise CredentialError("exactly one credential assignment is required")
        return assignments[0] + b"\n"
    except CredentialError:
        raise
    except (ValueError, UnicodeError):
        raise CredentialError("unsupported profile syntax") from None


def _read_profile(profile: str | os.PathLike[str], restrict: bool,
                  assignment_line: int | None = None) -> tuple[bytes, bool]:
    with _parent(profile, destination=False) as (parent, name):
        before = os.stat(name, dir_fd=parent, follow_symlinks=False)
        _check_file(before, profile=True, restrict=restrict)
        fd = os.open(name, _FILE_FLAGS, dir_fd=parent)
        try:
            if _identity(before) != _identity(os.fstat(fd)):
                raise CredentialError("profile changed during operation")
            _no_acl(fd)
            _verify_name(fd, parent, name)
            restricted = stat.S_IMODE(before.st_mode) != 0o600
            if restricted:
                os.fchmod(fd, 0o600)
                os.fsync(fd)
                after = os.fstat(fd)
                if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
                        after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
                    raise CredentialError("profile changed during permission restriction")
                _verify_name(fd, parent, name)
            _check_file(os.fstat(fd))
            before = os.fstat(fd)
            content = _read_bounded(fd, MAX_PROFILE_BYTES)
            if _identity(before) != _identity(os.fstat(fd)):
                raise CredentialError("profile changed during read")
            _verify_name(fd, parent, name)
            return _parse_profile(content, assignment_line), restricted
        finally:
            os.close(fd)


def _existing_matches(parent: int, name: str, content: bytes) -> bool:
    try:
        before = os.stat(name, dir_fd=parent, follow_symlinks=False)
    except FileNotFoundError:
        return False
    _check_file(before)
    fd = os.open(name, _FILE_FLAGS, dir_fd=parent)
    try:
        if _identity(before) != _identity(os.fstat(fd)):
            raise CredentialError("credential file changed during operation")
        _no_acl(fd)
        existing = _read_bounded(fd, MAX_TOKEN_BYTES + 1)
        if _identity(before) != _identity(os.fstat(fd)):
            raise CredentialError("credential file changed during read")
        _verify_name(fd, parent, name)
        if not hmac.compare_digest(existing, content):
            raise CredentialError("existing credential differs; overwrite is forbidden")
        return True
    finally:
        os.close(fd)


def _publish(parent: int, name: str, content: bytes) -> str:
    if _existing_matches(parent, name, content):
        return "unchanged"
    temporary = ".brave-credential-" + secrets.token_hex(16)
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                 | os.O_CLOEXEC, 0o600, dir_fd=parent)
    try:
        os.fchmod(fd, 0o600)
        _no_acl(fd)
        remaining = memoryview(content)
        while remaining:
            written = os.write(fd, remaining)
            if written <= 0:
                raise CredentialError("unable to write credential")
            remaining = remaining[written:]
        os.fsync(fd)
        _verify_name(fd, parent, temporary)
        _check_directory(parent, immediate=True, private=True)
        try:
            # link() publishes the fully synced inode atomically and cannot
            # overwrite a concurrent winner, unlike rename()/replace().
            os.link(temporary, name, src_dir_fd=parent, dst_dir_fd=parent,
                    follow_symlinks=False)
            result = "installed"
        except FileExistsError:
            if not _existing_matches(parent, name, content):
                raise CredentialError("credential destination changed during operation") from None
            result = "unchanged"
    finally:
        try:
            _verify_name(fd, parent, temporary)
            os.unlink(temporary, dir_fd=parent)
            os.fsync(parent)
        finally:
            os.close(fd)
    return result


@_redacted_errors
def install_credentials(profile: str | os.PathLike[str],
                        destination: str | os.PathLike[str] | None = None, *,
                        restrict_profile_permissions: bool = False,
                        assignment_line: int | None = None) -> dict[str, str | bool]:
    """Install only from the explicitly selected profile; never return its value."""
    _uid()
    destination = default_destination() if destination is None else destination
    _path_parts(destination)
    content, restricted = _read_profile(profile, restrict_profile_permissions, assignment_line)
    with _parent(destination, destination=True, create=True) as (parent, name):
        state = _publish(parent, name, content)
    return {"state": state, "profile_permissions_restricted": restricted}


@_redacted_errors
def inspect_credentials(destination: str | os.PathLike[str] | None = None) -> dict[str, str]:
    """Content-free, read-only metadata check. This does not verify a usable key."""
    _uid()
    destination = default_destination() if destination is None else destination
    try:
        with _parent(destination, destination=True) as (parent, name):
            info = os.stat(name, dir_fd=parent, follow_symlinks=False)
            _check_file(info)
            fd = os.open(name, _FILE_FLAGS, dir_fd=parent)
            try:
                if _identity(info) != _identity(os.fstat(fd)):
                    raise CredentialError("credential file changed during inspection")
                _no_acl(fd)
                _verify_name(fd, parent, name)
            finally:
                os.close(fd)
            return {"state": "present", "mode": "0600"}
    except FileNotFoundError:
        return {"state": "absent"}


class _Parser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise CredentialError("invalid command-line arguments")


def main(argv: list[str] | None = None) -> int:
    parser = _Parser(prog="brave_search_credentials.py", description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    install = commands.add_parser("install")
    install.add_argument("--profile", required=True)
    install.add_argument("--destination")
    install.add_argument("--restrict-profile-permissions", action="store_true")
    install.add_argument("--assignment-line", type=int,
                         help="extract this one-based literal line, not the sourced environment")
    inspect = commands.add_parser("status", aliases=["inspect"])
    inspect.add_argument("--destination")
    try:
        args = parser.parse_args(argv)
        if args.command == "install":
            result = install_credentials(
                args.profile, args.destination,
                restrict_profile_permissions=args.restrict_profile_permissions,
                assignment_line=args.assignment_line,
            )
        else:
            result = inspect_credentials(args.destination)
        print(json.dumps(result, sort_keys=True))
        return 0
    except CredentialError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    except (Exception, KeyboardInterrupt):
        print("error: credential operation failed", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
