#!/usr/bin/env python3
"""Publish only a complete verified build whose tag still names this commit.

Existing releases are hidden as drafts while replacing their complete asset set.
Failed uploads or checksum verification leave the release in draft. This script
never deletes a release, changes a Git ref, or silently removes unexpected assets.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import tempfile
import time
from types import SimpleNamespace
from urllib.parse import quote

SPEC = importlib.util.spec_from_file_location("release_tools", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)
require = release.require


class RetryableUploadError(ValueError):
    pass


class GitHub:
    def __init__(self, repository):
        require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository), "invalid repository")
        self.repository = repository
        self.base = f"repos/{repository}"

    def api(self, path, *, method="GET", data=None, missing_ok=False):
        command = ["gh", "api", f"{self.base}/{path}", "--method", method]
        if data is not None:
            command += ["--input", "-"]
        result = subprocess.run(command, input=json.dumps(data) if data is not None else None,
                                text=True, capture_output=True, check=False)
        if result.returncode:
            if missing_ok and "(HTTP 404)" in result.stderr:
                return None
            raise ValueError(f"GitHub {method} failed for {path}; refusing to continue")
        return json.loads(result.stdout)

    def current_commit(self, tag):
        value = self.api(f"git/ref/tags/{quote(tag, safe='')}")["object"]
        seen = set()
        while value["type"] == "tag":
            require(value["sha"] not in seen and len(seen) < 8, "invalid annotated tag chain")
            seen.add(value["sha"])
            value = self.api(f"git/tags/{value['sha']}")["object"]
        require(value["type"] == "commit", "release tag does not point to a commit")
        return value["sha"]

    def get_release(self, tag):
        # The by-tag REST endpoint only promises published releases. A failed
        # earlier rebuild may have left a draft, which must be found and reused.
        published = self.api(f"releases/tags/{quote(tag, safe='')}", missing_ok=True)
        if published is not None:
            require(published.get("tag_name") == tag, "release tag mismatch")
            return published
        matches, page = [], 1
        while True:
            values = self.api(f"releases?per_page=100&page={page}")
            require(isinstance(values, list), "invalid release list response")
            matches.extend(item for item in values if item.get("tag_name") == tag)
            require(len(matches) <= 1, "ambiguous releases for the same tag; refusing to create or replace")
            if len(values) < 100:
                return matches[0] if matches else None
            page += 1

    def get_release_by_id(self, identifier):
        return self.api(f"releases/{identifier}")

    def create_release(self, data):
        return self.api("releases", method="POST", data=data)

    def edit_release(self, identifier, data):
        return self.api(f"releases/{identifier}", method="PATCH", data=data)

    def assets(self, identifier):
        result, page = [], 1
        while True:
            values = self.api(f"releases/{identifier}/assets?per_page=100&page={page}")
            require(isinstance(values, list), "invalid release asset response")
            result.extend(values)
            if len(values) < 100:
                return result
            page += 1

    def upload(self, tag, path):
        # --clobber replaces one known asset. The entire release is already a
        # draft, so downloaders cannot see a mixture of old and new packages.
        result = subprocess.run(["gh", "release", "upload", tag, str(path.resolve()),
                                 "--repo", self.repository, "--clobber"],
                                check=False, text=True, capture_output=True)
        if result.returncode:
            transient = re.search(r"HTTP (?:5\d\d|408|429)\b|ReleaseAsset\.name already exists|i/o timeout|TLS handshake timeout|connection reset",
                                  result.stderr)
            error = RetryableUploadError if transient else ValueError
            raise error(f"release upload failed; release remains a draft: {result.stderr.strip()}")


def check_current(github, tag, commit):
    require(re.fullmatch(r"[0-9a-f]{40}", commit), "expected full git commit")
    require(github.current_commit(tag) == commit,
            "stale release run: remote tag no longer points to this commit")


def validate_assets(args):
    """Re-run all aggregate gates before any API mutation, then compare bytes."""
    require(args.assets.is_dir() and not args.assets.is_symlink(), "missing release assets")
    with tempfile.TemporaryDirectory(prefix="mediaatlas-publish-verify-") as temporary:
        output = Path(temporary) / "release-assets"
        release.aggregate(SimpleNamespace(tag=args.tag, commit=args.commit,
                                         repository=args.repository, input=args.input, output=output))
        expected = {path.name for path in output.iterdir()}
        require({path.name for path in args.assets.iterdir()} == expected,
                "unexpected/missing aggregate release files")
        for path in args.assets.iterdir():
            require(path.is_file() and not path.is_symlink(), "unsafe release asset")
            require(release.sha256(path) == release.sha256(output / path.name),
                    f"aggregate asset changed: {path.name}")
    return sorted(args.assets.iterdir())


def asset_matches(asset, path):
    return (asset.get("state") == "uploaded" and asset.get("size") == path.stat().st_size
            and asset.get("digest") == f"sha256:{release.sha256(path)}")


def verify_remote_assets(github, identifier, paths):
    expected = {path.name: path for path in paths}
    values = github.assets(identifier)
    require(len(values) == len(expected) and {asset["name"] for asset in values} == set(expected),
            "incomplete or mixed remote release asset set; release remains a draft")
    for asset in values:
        path = expected[asset["name"]]
        require(asset_matches(asset, path),
                f"remote asset verification failed: {path.name}; release remains a draft")


def publish_verified(github, tag, commit, paths):
    """Execute guarded mutations only after validate_assets has succeeded."""
    check_current(github, tag, commit)
    existing = github.get_release(tag)
    expected = {path.name for path in paths}
    if existing is not None:
        require(existing.get("immutable") is not True, "immutable release cannot be rebuilt")
        identifier = existing["id"]
        old_assets = github.assets(identifier)
        require({item["name"] for item in old_assets} <= expected,
                "unexpected existing assets; refuse to delete or mix release packages")
        check_current(github, tag, commit)
        github.edit_release(identifier, {"draft": True})
    else:
        check_current(github, tag, commit)
        existing = github.create_release({"tag_name": tag, "target_commitish": commit,
                                           "name": f"MediaAtlas {tag}", "draft": True,
                                           "prerelease": True, "make_latest": "false"})
        identifier = existing["id"]
    for path in paths:
        for attempt in range(3):
            current = github.get_release_by_id(identifier)
            require(current is not None and current["id"] == identifier and current.get("tag_name") == tag
                    and current["draft"] is True,
                    "release ownership/draft state changed during upload")
            check_current(github, tag, commit)
            try:
                github.upload(tag, path)
                break
            except RetryableUploadError:
                check_current(github, tag, commit)
                matches = [asset for asset in github.assets(identifier) if asset.get("name") == path.name]
                require(len(matches) <= 1, "ambiguous remote asset; release remains a draft")
                # An upload can reach GitHub even when the client reports an error.
                if matches and asset_matches(matches[0], path):
                    break
                if attempt == 2:
                    raise
                time.sleep(2 ** attempt)
    check_current(github, tag, commit)
    verify_remote_assets(github, identifier, paths)
    current = github.get_release_by_id(identifier)
    require(current is not None and current["id"] == identifier and current.get("tag_name") == tag
            and current["draft"] is True,
            "release ownership/draft state changed before publication")
    notes = next(path for path in paths if path.name == "RELEASE_NOTES.md").read_text()
    # This is a fresh read immediately before PATCH, not an atomic tag/ref CAS.
    check_current(github, tag, commit)
    github.edit_release(identifier, {"draft": False, "prerelease": True,
                                     "make_latest": "false", "name": f"MediaAtlas {tag}",
                                     "target_commitish": commit, "body": notes})
    print(f"Published verified complete build {commit} for {tag}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check-current", "publish"])
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--input", type=Path)
    parser.add_argument("--assets", type=Path)
    args = parser.parse_args()
    release.version(args.tag)
    github = GitHub(args.repository)
    if args.command == "check-current":
        check_current(github, args.tag, args.commit)
    else:
        require(args.input is not None and args.assets is not None, "publish needs --input and --assets")
        paths = validate_assets(args)
        publish_verified(github, args.tag, args.commit, paths)


if __name__ == "__main__":
    main()
