"""Guard repeated tag builds without executing GitHub mutations in tests."""
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("publisher", Path(__file__).with_name("publish.py"))
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)


class FakeGitHub:
    def __init__(self, commit, existing=True):
        self.commit = commit
        self.release = {"id": 123, "draft": False, "tag_name": "v0.1.0-beta.2"} if existing else None
        self.uploaded = {}
        self.changes = []
        self.fail_upload = False
        self.corrupt_digest = False
        self.tag_after_upload = None
        self.extra_assets = []

    def current_commit(self, tag):
        return self.commit

    def get_release(self, tag):
        return self.release

    def get_release_by_id(self, identifier):
        return self.release

    def create_release(self, data):
        self.changes.append(("create", data))
        self.release = {"id": 123, **data}
        return self.release

    def edit_release(self, identifier, data):
        self.changes.append(("edit", data))
        self.release.update(data)
        return self.release

    def upload(self, tag, path):
        if not self.release["draft"]:
            raise AssertionError("mixed assets would be publicly visible")
        if self.fail_upload:
            raise ValueError("upload failed")
        self.changes.append(("upload", path.name))
        self.uploaded[path.name] = path
        if self.tag_after_upload:
            self.commit = self.tag_after_upload

    def assets(self, identifier):
        return self.extra_assets + [
            {"name": name, "size": path.stat().st_size, "state": "uploaded",
             "digest": "sha256:" + ("0" * 64 if self.corrupt_digest else publisher.release.sha256(path))}
            for name, path in self.uploaded.items()
        ]


class PublishTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.commit = "1" * 40
        self.tag = "v0.1.0-beta.2"
        self.paths = []
        for name in ["MediaAtlas.exe", "RELEASE_NOTES.md", "SHA256SUMS"]:
            path = self.root / name
            path.write_text(f"Verified fixture {name}\n")
            self.paths.append(path)

    def publish(self, github):
        publisher.publish_verified(github, self.tag, self.commit, self.paths)

    def test_rebuild_reuses_existing_release_and_drafts_before_replacing_all_assets(self):
        github = FakeGitHub(self.commit)
        self.publish(github)
        self.assertEqual(github.changes[0], ("edit", {"draft": True}))
        self.assertFalse(any(name == "create" for name, _ in github.changes))
        self.assertEqual([value for name, value in github.changes if name == "upload"],
                         [path.name for path in self.paths])
        self.assertFalse(github.changes[-1][1]["draft"])
        self.assertEqual(github.changes[-1][1]["target_commitish"], self.commit)

    def test_new_tag_build_creates_draft_then_publishes_complete_set(self):
        github = FakeGitHub(self.commit, existing=False)
        self.publish(github)
        self.assertEqual(github.changes[0][0], "create")
        self.assertTrue(github.changes[0][1]["draft"])
        self.assertFalse(github.release["draft"])

    def test_already_moved_tag_is_rejected_without_any_changes(self):
        github = FakeGitHub("2" * 40)
        with self.assertRaisesRegex(ValueError, "stale release run"):
            self.publish(github)
        self.assertEqual(github.changes, [])

    def test_tag_moved_during_upload_stops_remaining_replacements_and_publication(self):
        github = FakeGitHub(self.commit)
        github.tag_after_upload = "2" * 40
        with self.assertRaisesRegex(ValueError, "stale release run"):
            self.publish(github)
        self.assertEqual(sum(name == "upload" for name, _ in github.changes), 1)
        self.assertTrue(github.release["draft"])

    def test_failed_upload_never_publishes_partial_asset_set(self):
        github = FakeGitHub(self.commit)
        github.fail_upload = True
        with self.assertRaisesRegex(ValueError, "upload failed"):
            self.publish(github)
        self.assertTrue(github.release["draft"])
        self.assertEqual(github.changes, [("edit", {"draft": True})])

    def test_wrong_remote_digest_keeps_release_draft(self):
        github = FakeGitHub(self.commit)
        github.corrupt_digest = True
        with self.assertRaisesRegex(ValueError, "remote asset verification"):
            self.publish(github)
        self.assertTrue(github.release["draft"])

    def test_tag_move_after_remote_verification_still_blocks_publication(self):
        github = FakeGitHub(self.commit)
        def verified(*_):
            github.commit = "2" * 40
        with patch.object(publisher, "verify_remote_assets", side_effect=verified), self.assertRaisesRegex(ValueError, "stale release run"):
            self.publish(github)
        self.assertTrue(github.release["draft"])

    def test_release_replaced_during_rebuild_blocks_uploads(self):
        github = FakeGitHub(self.commit)
        with patch.object(github, "get_release_by_id", return_value={"id": 999, "draft": True, "tag_name": self.tag}), self.assertRaisesRegex(ValueError, "ownership/draft state"):
            self.publish(github)
        self.assertFalse(any(name == "upload" for name, _ in github.changes))

    def test_upload_command_replaces_only_the_given_verified_asset(self):
        github = publisher.GitHub("owner/repo")
        result = SimpleNamespace(returncode=0)
        with patch.object(publisher.subprocess, "run", return_value=result) as execute:
            github.upload(self.tag, self.paths[0])
        self.assertEqual(execute.call_args.args[0],
                         ["gh", "release", "upload", self.tag, str(self.paths[0].resolve()),
                          "--repo", "owner/repo", "--clobber"])

    def test_unknown_old_assets_are_not_deleted_or_mixed(self):
        github = FakeGitHub(self.commit)
        github.extra_assets = [{"name": "unknown-old-package.zip"}]
        with self.assertRaisesRegex(ValueError, "unexpected existing assets"):
            self.publish(github)
        self.assertEqual(github.changes, [])

    def test_immutable_release_is_not_modified(self):
        github = FakeGitHub(self.commit)
        github.release["immutable"] = True
        with self.assertRaisesRegex(ValueError, "immutable"):
            self.publish(github)
        self.assertEqual(github.changes, [])

    def test_lightweight_and_nested_annotated_tags_resolve_to_actual_commit(self):
        github = publisher.GitHub("owner/repo")
        for responses in [
            [{"object": {"type": "commit", "sha": self.commit}}],
            [{"object": {"type": "tag", "sha": "a" * 40}},
             {"object": {"type": "tag", "sha": "b" * 40}},
             {"object": {"type": "commit", "sha": self.commit}}],
        ]:
            with self.subTest(responses=responses), patch.object(github, "api", side_effect=responses):
                self.assertEqual(github.current_commit(self.tag), self.commit)

    def test_annotated_tag_cycles_fail_closed(self):
        github = publisher.GitHub("owner/repo")
        response = {"object": {"type": "tag", "sha": "a" * 40}}
        with patch.object(github, "api", return_value=response), self.assertRaisesRegex(ValueError, "tag chain"):
            github.current_commit(self.tag)

    def test_http_auth_error_cannot_be_mistaken_for_missing_release(self):
        github = publisher.GitHub("owner/repo")
        result = SimpleNamespace(returncode=1, stdout="", stderr="gh: Bad credentials (HTTP 401)")
        with patch.object(publisher.subprocess, "run", return_value=result), self.assertRaisesRegex(ValueError, "GitHub GET failed"):
            github.get_release(self.tag)

    def test_missing_release_is_distinguished_from_auth_failure(self):
        github = publisher.GitHub("owner/repo")
        missing = SimpleNamespace(returncode=1, stdout="", stderr="gh: Not Found (HTTP 404)")
        empty_list = SimpleNamespace(returncode=0, stdout="[]", stderr="")
        with patch.object(publisher.subprocess, "run", side_effect=[missing, empty_list]) as execute:
            self.assertIsNone(github.get_release(self.tag))
        self.assertEqual(execute.call_count, 2)

    def test_published_and_failed_draft_rebuilds_use_id_checks_when_tag_endpoint_returns_404(self):
        for already_draft in [False, True]:
            with self.subTest(already_draft=already_draft):
                backend = FakeGitHub(self.commit)
                backend.release["draft"] = already_draft
                github = publisher.GitHub("owner/repo")
                calls = []
                def api(path, *, method="GET", data=None, missing_ok=False):
                    calls.append((path, method))
                    if path.startswith("git/ref/tags/"):
                        return {"object": {"type": "commit", "sha": self.commit}}
                    if path.startswith("releases/tags/"):
                        # Match the documented REST boundary: draft is not found.
                        return None if backend.release["draft"] else dict(backend.release)
                    if path.startswith("releases?per_page=100&page="):
                        return [dict(backend.release)]
                    if path.startswith("releases/123/assets?"):
                        return backend.assets(123)
                    if path == "releases/123" and method == "PATCH":
                        return backend.edit_release(123, data)
                    if path == "releases/123":
                        return dict(backend.release)
                    raise AssertionError(f"Unexpected API or duplicate create: {method} {path}")
                with patch.object(github, "api", side_effect=api), patch.object(github, "upload", side_effect=backend.upload):
                    self.publish(github)
                self.assertFalse(backend.release["draft"])
                self.assertEqual(sum(path.startswith("releases/tags/") for path, _ in calls), 1)
                self.assertEqual(sum(path == "releases/123" and method == "GET" for path, method in calls), len(self.paths) + 1)
                self.assertFalse(any(name == "create" for name, _ in backend.changes))
                # The final ref check follows the ownership read, immediately
                # preceding the final mutation, without implying an atomic CAS.
                self.assertTrue(calls[-2][0].startswith("git/ref/tags/"))
                self.assertEqual(calls[-1], ("releases/123", "PATCH"))

    def test_draft_lookup_searches_all_release_pages(self):
        github = publisher.GitHub("owner/repo")
        unrelated = [{"id": number, "tag_name": f"v0.0.{number}", "draft": False} for number in range(100)]
        draft = {"id": 123, "tag_name": self.tag, "draft": True}
        with patch.object(github, "api", side_effect=[None, unrelated, [draft]]) as api:
            self.assertEqual(github.get_release(self.tag), draft)
        self.assertEqual(api.call_args.args[0], "releases?per_page=100&page=2")

    def test_ambiguous_same_tag_drafts_cannot_create_or_replace_a_release(self):
        github = publisher.GitHub("owner/repo")
        drafts = [{"id": value, "tag_name": self.tag, "draft": True} for value in [123, 456]]
        with patch.object(github, "api", side_effect=[None, drafts]), self.assertRaisesRegex(ValueError, "ambiguous releases"):
            github.get_release(self.tag)

    def test_id_lookup_with_changed_tag_cannot_upload_assets(self):
        github = FakeGitHub(self.commit)
        with patch.object(github, "get_release_by_id", return_value={"id": 123, "tag_name": "different-tag", "draft": True}), self.assertRaisesRegex(ValueError, "ownership/draft state"):
            self.publish(github)
        self.assertFalse(any(name == "upload" for name, _ in github.changes))

    def test_real_complete_aggregate_is_revalidated_with_identical_asset_bytes(self):
        source = self.root / "source"
        source.mkdir()
        for filename in ["package.json", "apps/desktop/package.json", "apps/desktop/src-tauri/tauri.conf.json"]:
            path = source / filename
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"version": self.tag[1:]}))
        (source / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{self.tag[1:]}"\n')
        (source / "Cargo.lock").write_text('package = []\n')
        for filename in ["LICENSE-STATUS.md", "THIRD_PARTY_NOTICES.md", "docs/dependency-licenses.json", "docs/third-party-licenses/texts/license.txt"]:
            path = source / filename
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("Fixture license material.\n")
        targets = self.root / "targets"
        for spec in publisher.release.TARGETS:
            if not spec["required"]:
                continue
            directory = targets / spec["target"]
            directory.mkdir(parents=True)
            entries = []
            for suffix in spec["formats"]:
                filename = f"MediaAtlas_{self.tag[1:]}_{spec['target']}.{suffix}"
                path = directory / filename
                path.write_bytes(b"verified native fixture")
                entries.append({"name": filename, "format": suffix, "bytes": path.stat().st_size,
                                "sha256": publisher.release.sha256(path), "internal_architecture_verified": True,
                                "package_verification": {"macos_signature_verified": True} if spec["os"] == "macos" else {}})
            manifest = {"schema": 1, "tag": self.tag, "version": self.tag[1:], "commit": self.commit,
                        **spec, "tests_passed": True, "assets": entries}
            (directory / "manifest.json").write_text(json.dumps(manifest))
        args = SimpleNamespace(tag=self.tag, commit=self.commit, repository="owner/repo", input=targets,
                               output=self.root / "aggregate", assets=self.root / "aggregate")
        with patch.object(publisher.release, "ROOT", source):
            publisher.release.aggregate(args)
            paths = publisher.validate_assets(args)
        self.assertIn("SHA256SUMS", [path.name for path in paths])
        self.assertEqual(len(paths), len(list(args.assets.iterdir())))

    def test_changed_local_asset_is_rejected_before_remote_changes(self):
        # Aggregate performs the native provenance checks; this regression checks
        # that publication cannot trust an asset directory changed afterwards.
        assets = self.root / "assets"
        assets.mkdir()
        (assets / "package.exe").write_bytes(b"tampered package")
        def aggregate(args):
            args.output.mkdir()
            (args.output / "package.exe").write_bytes(b"verified package")
        args = SimpleNamespace(tag=self.tag, commit=self.commit, repository="owner/repo",
                               input=self.root / "input", assets=assets)
        with patch.object(publisher.release, "aggregate", side_effect=aggregate), self.assertRaisesRegex(ValueError, "asset changed"):
            publisher.validate_assets(args)

    def test_publication_reuses_complete_aggregate_provenance_gate(self):
        args = SimpleNamespace(tag=self.tag, commit=self.commit, repository="owner/repo",
                               input=self.root / "input", assets=self.root)
        with patch.object(publisher.release, "aggregate", side_effect=ValueError("missing required target")), self.assertRaisesRegex(ValueError, "missing required target"):
            publisher.validate_assets(args)


if __name__ == "__main__":
    unittest.main()
