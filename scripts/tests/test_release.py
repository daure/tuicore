import importlib.util
import hashlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import call, patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import release


class ReleaseTests(unittest.TestCase):
    def test_run_disables_pagers(self):
        with patch.object(release.subprocess, "run") as subprocess_run:
            release.run("git", "status", env={"GIT_EDITOR": "true"})
        environment = subprocess_run.call_args.kwargs["env"]
        self.assertEqual(environment["PAGER"], "cat")
        self.assertEqual(environment["GIT_PAGER"], "cat")
        self.assertEqual(environment["CARGO_PAGER"], "cat")
        self.assertEqual(environment["GIT_EDITOR"], "true")

    def test_stable_version_bumps(self):
        for bump, expected in [("patch", "0.40.1"), ("minor", "0.41.0"), ("major", "1.0.0")]:
            self.assertEqual(release.next_version("0.40.0", bump), expected)
        with self.assertRaises(ValueError):
            release.next_version("0.40.0-rc.1", "patch")

    def test_dirty_tree_stops_before_network_or_publication(self):
        with patch.object(release.os, "chdir"), patch.object(release, "output", return_value=" M Cargo.toml"), patch.object(release, "registry_version") as registry:
            with self.assertRaisesRegex(ValueError, "working tree must be clean"):
                release.release("patch")
            registry.assert_not_called()

    def test_registry_cargo_ignores_local_config(self):
        original_cwd = Path.cwd()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = root / "repo"
            repo.mkdir()
            personal_home = root / "personal-cargo"
            (personal_home / "registry").mkdir(parents=True)
            try:
                os.chdir(repo)
                with patch.dict(os.environ, {"CARGO_HOME": str(personal_home)}), patch.object(release, "run") as run:
                    release.registry_cargo("metadata", "--locked")
                    first = run.call_args
                    home = Path(first.kwargs["env"]["CARGO_HOME"])
                    (home / "cache-marker").touch()
                    release.registry_cargo("metadata", "--locked")
                args, kwargs = run.call_args
                self.assertEqual(args[:3], ("cargo", "metadata", "--manifest-path"))
                self.assertEqual(args[-1], "--locked")
                self.assertEqual(home, repo / "target/release-check/cargo-home")
                self.assertTrue((home / "cache-marker").exists())
                self.assertEqual((home / "registry").resolve(), personal_home / "registry")
                self.assertNotIn(repo, Path(kwargs["cwd"]).parents)
                self.assertNotIn(Path.home(), Path(kwargs["cwd"]).parents)
                self.assertEqual(first.kwargs["env"], kwargs["env"])
                self.assertEqual(kwargs["env"]["CARGO_TARGET_DIR"], str(repo / "target/release-check/build"))
                self.assertEqual(kwargs["env"]["CARGO_BUILD_JOBS"], "2")
                self.assertEqual(kwargs["env"]["CARGO_PROFILE_DEV_DEBUG"], "0")
                self.assertEqual(kwargs["env"]["CARGO_PROFILE_TEST_DEBUG"], "0")
                self.assertEqual(kwargs["env"]["CARGO_INCREMENTAL"], "0")
            finally:
                os.chdir(original_cwd)

    def test_cargo_checks_reuse_artifacts_without_personal_or_project_config(self):
        original_cwd = Path.cwd()
        real_run = release.run
        results = []

        def capture(*args, **kwargs):
            result = real_run(*args, capture_output=True, **kwargs)
            results.append(result)
            return result

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = root / "repo"
            (repo / "src").mkdir(parents=True)
            (repo / "Cargo.toml").write_text('[package]\nname = "cache-probe"\nversion = "0.1.0"\nedition = "2024"\n')
            (repo / "src/main.rs").write_text("fn main() {}\n")
            personal_home = root / "personal-cargo"
            for config in (personal_home, root / ".cargo", repo / ".cargo"):
                config.mkdir()
                (config / "config.toml").write_text('invalid TOML: this config must not be loaded\n')
            environment = {
                "CARGO_HOME": str(personal_home),
                "CARGO_TARGET_DIR": str(root / "personal-target"),
                "CARGO_BUILD_JOBS": "8",
                "CARGO_TERM_COLOR": "never",
            }
            try:
                os.chdir(repo)
                with patch.dict(os.environ, environment), patch.object(release, "run", side_effect=capture):
                    release.registry_cargo("check", "--offline", "--verbose")
                    release.registry_cargo("check", "--offline", "--verbose")
                self.assertIn("Fresh cache-probe", results[-1].stderr)
                self.assertTrue((repo / "target/release-check/build/debug").is_dir())
                self.assertFalse((root / "personal-target").exists())
            finally:
                os.chdir(original_cwd)


class ReleaseGitTests(unittest.TestCase):
    def test_release_pushes_matching_commit_and_tag_without_building(self):
        environment = {
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "Release test",
            "GIT_AUTHOR_EMAIL": "release@example.invalid",
            "GIT_COMMITTER_NAME": "Release test",
            "GIT_COMMITTER_EMAIL": "release@example.invalid",
        }
        original_cwd = Path.cwd()
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, environment):
            root = Path(directory)
            repo = root / "repo"
            repo.mkdir()
            (repo / "scripts").mkdir()
            (repo / "Cargo.toml").write_text('[package]\nname = "tuicore"\nversion = "0.40.0"\n')
            (repo / "Cargo.lock").write_text("version = 4\n")

            def git(*args):
                return subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True, text=True).stdout.strip()

            git("init", "--initial-branch=main")
            git("add", "Cargo.toml", "Cargo.lock")
            git("commit", "-m", "Initial")
            git("init", "--bare", str(root / "remote.git"))
            git("remote", "add", "origin", str(root / "remote.git"))
            git("push", "origin", "main")
            real_run = release.run
            calls = []

            def run(*args, **kwargs):
                calls.append((args, kwargs))
                if args[:2] == ("gh", "auth"):
                    return subprocess.CompletedProcess(args, 0)
                if args[0] in ("cargo", "python3"):
                    return subprocess.CompletedProcess(args, 0)
                return real_run(*args, **kwargs)

            try:
                with patch.object(release, "__file__", str(repo / "scripts/release.py")), patch.object(release, "run", side_effect=run), patch.object(release, "registry_version", return_value=None), patch.object(release, "registry_cargo") as cargo:
                    release.release("patch")
                self.assertIn('version = "0.40.1"', (repo / "Cargo.toml").read_text())
                self.assertEqual(git("rev-parse", "HEAD"), git("rev-parse", "v0.40.1^{commit}"))
                self.assertIn(git("rev-parse", "HEAD"), git("ls-remote", "origin", "refs/heads/main"))
                self.assertIn("refs/tags/v0.40.1", git("ls-remote", "origin", "refs/tags/v0.40.1"))
                self.assertIn(
                    (("git", "push", "--atomic", "origin", "HEAD:refs/heads/main", "refs/tags/v0.40.1"), {}),
                    calls,
                )
                tag_call = next(
                    call
                    for call in calls
                    if call[0] == ("git", "tag", "-a", "v0.40.1", "-m", "release: v0.40.1")
                )
                self.assertEqual(tag_call[1]["env"], {"GIT_EDITOR": "true"})
                self.assertIn(
                    (("python3", "-m", "unittest", "discover", "-s", "scripts/tests"), {}),
                    calls,
                )
                cargo.assert_has_calls(
                    [
                        call("clippy", "--locked", "--all-targets", "--", "-D", "warnings"),
                        call("test", "--locked", "--", "--test-threads=2"),
                        call("update", "--workspace"),
                    ]
                )
                self.assertEqual(git("status", "--porcelain"), "")
            finally:
                os.chdir(original_cwd)


class PublishTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("publish_crate", SCRIPTS / "publish-crate.py")
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)

    def test_retry_skips_published_version(self):
        with patch.object(self.module, "registry_version", return_value={"yanked": False}), patch.object(self.module.subprocess, "run") as run:
            self.module.main()
            run.assert_not_called()

    def test_yanked_version_blocks_release(self):
        with patch.object(self.module, "registry_version", return_value={"yanked": True}), patch.object(self.module.subprocess, "run") as run:
            with self.assertRaisesRegex(SystemExit, "is yanked"):
                self.module.main()
            run.assert_not_called()

    def test_publication_failure_propagates(self):
        with patch.object(self.module, "registry_version", return_value=None), patch.dict(self.module.os.environ, {"CARGO_REGISTRY_TOKEN": "test"}), patch.object(self.module.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "cargo")):
            with self.assertRaises(subprocess.CalledProcessError):
                self.module.main()


class PublishedCrateTests(unittest.TestCase):
    def test_only_verified_bytes_are_saved(self):
        spec = importlib.util.spec_from_file_location("fetch_crate", SCRIPTS / "fetch-crate.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        data = b"published package"
        checksum = hashlib.sha256(data).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            with patch.object(module, "registry_version", return_value={"yanked": False, "checksum": checksum}), patch.object(module.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                module.fetch_crate("0.40.1", destination)
            self.assertEqual((destination / "tuicore-0.40.1.crate").read_bytes(), data)
            self.assertIn(checksum, (destination / "tuicore-0.40.1.crate.sha256").read_text())
            with patch.object(module, "registry_version", return_value={"yanked": False, "checksum": "wrong"}), patch.object(module.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                with self.assertRaisesRegex(ValueError, "checksum"):
                    module.fetch_crate("0.40.2", destination)
            self.assertFalse((destination / "tuicore-0.40.2.crate").exists())


if __name__ == "__main__":
    unittest.main()
