#!/usr/bin/env python3
"""Synthetic-only tests. Never inspect a real profile or credential file."""

from __future__ import annotations

import contextlib
import io
import json
import os
from pathlib import Path
import pwd
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import brave_search_credentials as credentials


KEY = "SYNTHETIC-Brave_Only.123+/="
OTHER_KEY = "SYNTHETIC-Different-456"


class CredentialTests(unittest.TestCase):
    def setUp(self) -> None:
        # /tmp is intentionally rejected as a writable ancestor, including sticky
        # /tmp. Keep ephemeral fixtures under the account-owned home instead.
        home = Path(pwd.getpwuid(os.getuid()).pw_dir).resolve()
        self.temporary = tempfile.TemporaryDirectory(prefix=".brave-synthetic-", dir=home)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.profile = self.root / "profile"
        self.destination = self.root / "config" / "credentials" / "brave-key"
        self.write_profile(f"export {credentials.NAME}='{KEY}'\n")

    def write_profile(self, text: str | bytes, mode: int = 0o600) -> bytes:
        content = text.encode() if isinstance(text, str) else text
        self.profile.write_bytes(content)
        self.profile.chmod(mode)
        return content

    def install(self, **kwargs):
        return credentials.install_credentials(self.profile, self.destination, **kwargs)

    def assert_rejected(self, function, *args, **kwargs) -> str:
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            with self.assertRaises(credentials.CredentialError) as raised:
                function(*args, **kwargs)
        output = stdout.getvalue() + stderr.getvalue() + str(raised.exception)
        self.assertNotIn(KEY, output)
        self.assertNotIn(OTHER_KEY, output)
        self.assertEqual(stdout.getvalue(), "")
        self.assertEqual(stderr.getvalue(), "")
        return str(raised.exception)

    def cli(self, *args):
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            result = credentials.main(list(args))
        self.assertNotIn(KEY, stdout.getvalue() + stderr.getvalue())
        self.assertNotIn(OTHER_KEY, stdout.getvalue() + stderr.getvalue())
        return result, stdout.getvalue(), stderr.getvalue()

    def test_literal_quotes_comments_and_unrelated_commands(self) -> None:
        for quote in ("", "'", '"'):
            for export in ("", "export "):
                with self.subTest(quote=quote, export=export):
                    content = self.write_profile(
                        f"# {credentials.NAME}=ignored\n"
                        "alias harmless='printf never-executed'\n"
                        f"  {export}{credentials.NAME}={quote}{KEY}{quote} # literal\n"
                    )
                    result = self.install()
                    self.assertIn(result["state"], {"installed", "unchanged"})
                    self.assertEqual(self.destination.read_bytes(), (KEY + "\n").encode("ascii"))
                    self.assertEqual(self.profile.read_bytes(), content)

    def test_modes_and_status(self) -> None:
        self.assertEqual(credentials.inspect_credentials(self.destination), {"state": "absent"})
        self.assertFalse(self.destination.parent.exists())
        self.assertEqual(self.install(), {"state": "installed", "profile_permissions_restricted": False})
        for directory in (self.destination.parent, self.destination.parent.parent):
            self.assertEqual(stat.S_IMODE(directory.stat().st_mode), 0o700)
        self.assertEqual(stat.S_IMODE(self.destination.stat().st_mode), 0o600)
        self.assertEqual(self.destination.stat().st_nlink, 1)
        with mock.patch.object(credentials.os, "read", side_effect=AssertionError("no content reads")):
            self.assertEqual(credentials.inspect_credentials(self.destination),
                             {"state": "present", "mode": "0600"})

    def test_default_is_account_config_not_environment(self) -> None:
        with mock.patch.object(credentials.pwd, "getpwuid", return_value=SimpleNamespace(pw_dir=str(self.root))), \
                mock.patch.dict(os.environ, {"HOME": "/untrusted", "XDG_CONFIG_HOME": "/untrusted"}):
            expected = self.root / ".config/astrid/credentials/brave-search-api-key"
            self.assertEqual(credentials.default_destination(), expected)
            self.assertEqual(credentials.install_credentials(self.profile)["state"], "installed")
            self.assertEqual(expected.read_bytes(), (KEY + "\n").encode())

    def test_restrict_is_explicit_and_preserves_bytes_inode_and_mtime(self) -> None:
        content = self.write_profile(
            f"# Unicode comment: caf\u00e9\nexport {credentials.NAME}=\"{KEY}\"\n", 0o644)
        before = self.profile.stat()
        with mock.patch.object(credentials.os, "read", side_effect=AssertionError("no insecure read")):
            self.assert_rejected(self.install)
        self.assertEqual(stat.S_IMODE(self.profile.stat().st_mode), 0o644)
        self.assertFalse(self.destination.exists())
        result = self.install(restrict_profile_permissions=True)
        self.assertTrue(result["profile_permissions_restricted"])
        after = self.profile.stat()
        self.assertEqual((before.st_dev, before.st_ino, before.st_mtime_ns),
                         (after.st_dev, after.st_ino, after.st_mtime_ns))
        self.assertEqual(self.profile.read_bytes(), content)
        self.assertEqual(stat.S_IMODE(after.st_mode), 0o600)
        self.assertFalse(self.install(restrict_profile_permissions=True)["profile_permissions_restricted"])

    def test_same_retry_preserves_destination_and_different_retry_is_refused(self) -> None:
        self.install()
        before = self.destination.stat()
        self.assertEqual(self.install()["state"], "unchanged")
        after = self.destination.stat()
        self.assertEqual(credentials._identity(before), credentials._identity(after))
        self.write_profile(f"{credentials.NAME}={OTHER_KEY}\n")
        self.assert_rejected(self.install)
        self.assertEqual(self.destination.read_bytes(), (KEY + "\n").encode())
        self.assertEqual(credentials._identity(after), credentials._identity(self.destination.stat()))

    def test_duplicate_missing_and_other_references(self) -> None:
        for text in (
            "# no assignment\n",
            f"{credentials.NAME}={KEY}\nexport {credentials.NAME}={KEY}\n",
            f"{credentials.NAME}={KEY}\n{credentials.NAME}+={OTHER_KEY}\n",
            f"{credentials.NAME}={KEY}\nunset {credentials.NAME}\n",
            f"{credentials.NAME}={KEY}\necho ${credentials.NAME}\n",
        ):
            with self.subTest(kind=text.splitlines()[-1][:10]):
                self.write_profile(text)
                self.assert_rejected(self.install)
                self.assertFalse(self.destination.exists())

    def test_substitution_commands_and_malformed_shell(self) -> None:
        forms = [
            f"{credentials.NAME}=$(echo {KEY})", f"{credentials.NAME}=`echo {KEY}`",
            f"{credentials.NAME}='$(echo {KEY})'", f'{credentials.NAME}="${{OTHER}}"',
            f"{credentials.NAME}=$OTHER", f"{credentials.NAME}=$'{KEY}'",
            f"{credentials.NAME}={KEY}; echo executed", f"{credentials.NAME}={KEY} command",
            f"{credentials.NAME}={KEY} OTHER=value", f"env {credentials.NAME}={KEY}",
            f"if true; then {credentials.NAME}={KEY}; fi", f"{credentials.NAME}=<{KEY}",
            f"{credentials.NAME}=>(echo {KEY})", f"{credentials.NAME}={KEY} > output",
            f"{credentials.NAME}={KEY} && true", f"{credentials.NAME}={KEY} | true",
            f"{credentials.NAME} = {KEY}", f"{credentials.NAME}= {KEY}",
            f"{credentials.NAME}='{KEY}", f'{credentials.NAME}="{KEY}',
            f"{credentials.NAME}='part'{KEY}", f"{credentials.NAME}=abc\\def",
            f"{credentials.NAME}={KEY}#not-a-shell-comment", f"{credentials.NAME}='two words'",
            f"{credentials.NAME}='line\n{KEY}'", f"{credentials.NAME}=\\\n{KEY}",
            f"cat <<EOF\n{credentials.NAME}={KEY}\nEOF", f"{credentials.NAME}=",
            f"{credentials.NAME}=\"\"", f"{credentials.NAME}=nonascii-\u00e9",
            f"{credentials.NAME}={KEY}\n'broken", f"{credentials.NAME}={KEY}\r\n",
        ]
        for index, form in enumerate(forms):
            with self.subTest(index=index):
                self.write_profile(form + "\n")
                self.assert_rejected(self.install)
                self.assertFalse(self.destination.exists())

    def test_invalid_encoding_nul_and_size_limits(self) -> None:
        for content in (b"\xff", b"\x00", b"x" * (credentials.MAX_PROFILE_BYTES + 1),
                        (credentials.NAME + "=" + "x" * 4097).encode()):
            self.write_profile(content)
            self.assert_rejected(self.install)

    def test_selected_literal_ignores_unrelated_multiline_without_execution(self) -> None:
        content = self.write_profile(
            "other='multiline\nquoted data'\n"
            "alias unrelated=\\\n'printf never-executed'\n"
            "# BRAVE_SEARCH_API_KEY in a whole-line comment is ignored\n"
            f"export {credentials.NAME}='{KEY}' # chosen literal\n"
            "unrelated='even incomplete shell is not evaluated\n")
        self.assert_rejected(self.install)
        self.assertEqual(self.install(assignment_line=6)["state"], "installed")
        self.assertEqual(self.profile.read_bytes(), content)
        self.assertEqual(self.destination.read_bytes(), (KEY + "\n").encode())
        args = ["install", "--profile", str(self.profile), "--destination", str(self.destination),
                "--assignment-line", "6"]
        self.assertEqual(self.cli(*args)[0], 0)

    def test_selected_literal_rejects_ambiguity_heredocs_and_continuations(self) -> None:
        for prefix, suffix in (
            (f"{credentials.NAME}={OTHER_KEY}\n", ""),
            (f"echo ${credentials.NAME}\n", ""),
            ("BRAVE_SEARCH_API_\\\nKEY=other\n", ""),
            ("cat <<EOF\n", "EOF\n"),
            ("cat <<'EOF'\n", "EOF\n"),
            ("cat <\\\n<EOF\n", "EOF\n"),
            ("unrelated='quoted << marker'\n", ""),
            ("command \\\n", ""),
            ("", f"export {credentials.NAME}={OTHER_KEY}\n"),
        ):
            with self.subTest(prefix_kind=prefix[:4]):
                content = prefix + f"{credentials.NAME}={KEY}\n" + suffix
                self.write_profile(content)
                self.assert_rejected(self.install, assignment_line=prefix.count("\n") + 1)
                self.assertFalse(self.destination.exists())

    def test_selected_line_requires_literal_not_command_and_valid_line_number(self) -> None:
        for value in (0, -1, 1000, True, "1"):
            self.assert_rejected(self.install, assignment_line=value)
        for expression in (f"$(echo {KEY})", f"'{KEY}'; command", "$OTHER", f"'{KEY}"):
            self.write_profile(f"{credentials.NAME}={expression}\n")
            self.assert_rejected(self.install, assignment_line=1)

    def test_symlink_profile_and_parent(self) -> None:
        actual = self.root / "actual-profile"
        self.profile.rename(actual)
        self.profile.symlink_to(actual)
        self.assert_rejected(self.install, restrict_profile_permissions=True)
        self.profile.unlink()
        self.profile = self.root / "profile-parent" / "actual-profile"
        self.profile.parent.symlink_to(self.root, target_is_directory=True)
        self.assert_rejected(self.install)

    def test_symlink_destination_and_parent(self) -> None:
        self.destination.parent.mkdir(parents=True, mode=0o700)
        self.destination.symlink_to(self.profile)
        self.assert_rejected(self.install)
        self.assert_rejected(credentials.inspect_credentials, self.destination)
        self.destination.unlink()
        link = self.root / "link"
        link.symlink_to(self.destination.parent, target_is_directory=True)
        self.assert_rejected(credentials.install_credentials, self.profile, link / "key")
        self.assert_rejected(credentials.inspect_credentials, link / "key")

    def test_hardlinks_and_nonregular_files(self) -> None:
        alias = self.root / "alias"
        os.link(self.profile, alias)
        self.assert_rejected(self.install, restrict_profile_permissions=True)
        alias.unlink()
        self.install()
        os.link(self.destination, alias)
        self.assert_rejected(self.install)
        self.assert_rejected(credentials.inspect_credentials, self.destination)
        alias.unlink()
        self.destination.unlink()
        os.mkfifo(self.destination, 0o600)
        self.assert_rejected(self.install)
        self.assert_rejected(credentials.inspect_credentials, self.destination)
        self.destination.unlink()
        self.destination.mkdir(mode=0o700)
        self.assert_rejected(self.install)

    def test_insecure_directories_and_existing_file_modes(self) -> None:
        self.destination.parent.mkdir(parents=True, mode=0o700)
        for mode in (0o755, 0o770, 0o777, 0o1700):
            with self.subTest(mode=mode):
                self.destination.parent.chmod(mode)
                self.assert_rejected(self.install)
        self.destination.parent.chmod(0o700)
        self.install()
        for mode in (0o644, 0o660, 0o700, 0o4600):
            with self.subTest(mode=mode):
                self.destination.chmod(mode)
                self.assert_rejected(self.install)
                self.assert_rejected(credentials.inspect_credentials, self.destination)

    def test_insecure_ancestor_and_profile_cannot_be_repaired(self) -> None:
        self.root.chmod(0o777)
        self.assert_rejected(self.install)
        self.root.chmod(0o700)
        for mode in (0o666, 0o620, 0o700, 0o1600):
            self.profile.chmod(mode)
            self.assert_rejected(self.install, restrict_profile_permissions=True)
            self.assertEqual(stat.S_IMODE(self.profile.stat().st_mode), mode)

    def test_foreign_owner_and_privilege_change(self) -> None:
        # Exercise owner checks without chown or elevated privileges.
        info = self.profile.stat()
        fake = SimpleNamespace(st_mode=info.st_mode, st_uid=os.getuid() + 10000, st_nlink=1)
        self.assert_rejected(credentials._check_file, fake)
        with mock.patch.object(credentials.os, "fstat", return_value=fake):
            self.assert_rejected(credentials._check_directory, 99, immediate=True)
        with mock.patch.object(credentials.os, "geteuid", return_value=os.getuid() + 1):
            self.assert_rejected(self.install)

    def test_malicious_paths_and_repository_destinations(self) -> None:
        for path in ("relative", "~/.config/key", str(self.root) + "/../key",
                     str(self.root) + "/./key", str(self.root) + "//key",
                     str(self.root) + "/$(touch bad)", str(self.root) + "/`bad`",
                     str(self.root) + "/key\n", str(self.root) + "/-option",
                     str(self.root) + "/key\x00", str(self.root) + "/.git/key"):
            with self.subTest(path_kind=path[-5:]):
                self.assert_rejected(credentials.install_credentials, self.profile, path)
                self.assert_rejected(credentials.inspect_credentials, path)
        for marker in (".git", ".hg", ".svn", "AGENTS.md"):
            (self.root / marker).write_text("synthetic repository marker")
            self.assert_rejected(self.install)
            (self.root / marker).unlink()
        self.assertFalse(self.destination.exists())

    def test_profile_replacement_before_chmod_is_rejected(self) -> None:
        self.profile.chmod(0o644)
        original_verify = credentials._verify_name
        replacement = self.root / "replacement"
        replacement.write_bytes(b"unrelated content\n")
        replacement.chmod(0o644)

        def swap(fd, parent, name):
            replacement.replace(self.profile)
            return original_verify(fd, parent, name)

        with mock.patch.object(credentials, "_verify_name", side_effect=swap), \
                mock.patch.object(credentials.os, "fchmod") as chmod:
            self.assert_rejected(self.install, restrict_profile_permissions=True)
            chmod.assert_not_called()
        self.assertEqual(self.profile.read_bytes(), b"unrelated content\n")
        self.assertEqual(stat.S_IMODE(self.profile.stat().st_mode), 0o644)

    def test_atomic_publish_sync_order_and_partial_writes(self) -> None:
        real_write, real_fsync, real_link = os.write, os.fsync, os.link
        calls = []

        def write(fd, data):
            calls.append("write")
            return real_write(fd, data[:3])

        def sync(fd):
            calls.append("file-sync" if stat.S_ISREG(os.fstat(fd).st_mode) else "directory-sync")
            return real_fsync(fd)

        def link(source, destination, **kwargs):
            calls.append("publish")
            self.assertFalse(self.destination.exists())
            self.assertIn("file-sync", calls)
            return real_link(source, destination, **kwargs)

        with mock.patch.object(credentials.os, "write", side_effect=write), \
                mock.patch.object(credentials.os, "fsync", side_effect=sync), \
                mock.patch.object(credentials.os, "link", side_effect=link):
            self.install()
        self.assertGreater(calls.count("write"), 1)
        self.assertEqual(calls[-1], "directory-sync")
        self.assertEqual(self.destination.read_bytes(), (KEY + "\n").encode())
        self.assertEqual(list(self.destination.parent.iterdir()), [self.destination])

    def test_interrupted_write_leaves_no_destination_or_temporary(self) -> None:
        with mock.patch.object(credentials.os, "write", side_effect=OSError(KEY)):
            self.assert_rejected(self.install)
        self.assertFalse(self.destination.exists())
        self.assertEqual(list(self.destination.parent.iterdir()), [])

    def test_concurrent_publication_same_or_different(self) -> None:
        real_link = credentials.os.link
        for winner in (KEY, OTHER_KEY):
            with self.subTest(same=winner == KEY):
                def publish_winner(source, destination, **kwargs):
                    fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                                 0o600, dir_fd=kwargs["dst_dir_fd"])
                    try:
                        os.write(fd, (winner + "\n").encode())
                    finally:
                        os.close(fd)
                    return real_link(source, destination, **kwargs)

                with mock.patch.object(credentials.os, "link", side_effect=publish_winner):
                    if winner == KEY:
                        self.assertEqual(self.install()["state"], "unchanged")
                    else:
                        self.assert_rejected(self.install)
                self.assertEqual(self.destination.read_bytes(), (winner + "\n").encode())
                self.assertEqual(list(self.destination.parent.iterdir()), [self.destination])
                self.destination.unlink()

    def test_cli_and_exception_redaction(self) -> None:
        args = ["install", "--profile", str(self.profile), "--destination", str(self.destination)]
        code, stdout, stderr = self.cli(*args)
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(stdout)["state"], "installed")
        self.assertEqual(stderr, "")
        self.write_profile(f"{credentials.NAME}={OTHER_KEY}\n")
        self.assertEqual(self.cli(*args)[0], 1)
        for verb in ("status", "inspect"):
            code, stdout, stderr = self.cli(verb, "--destination", str(self.destination))
            self.assertEqual(code, 0)
            self.assertEqual(json.loads(stdout), {"state": "present", "mode": "0600"})
        # Bad argv is never echoed by argparse, even when an operator misuses it.
        for arguments in (("install", "--key", KEY), (KEY,), ("install",)):
            self.assertEqual(self.cli(*arguments)[0], 1)
        with mock.patch.object(credentials, "_read_profile", side_effect=OSError(KEY)):
            error = self.assert_rejected(self.install)
            self.assertEqual(error, "credential operation failed")
            self.assertEqual(self.cli(*args)[0], 1)


if __name__ == "__main__":
    unittest.main()
