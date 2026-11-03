#!/usr/bin/env python3
"""Run Cargo unchanged; publish only allowlisted failure metadata to GitHub.

The raw log is streamed to the existing step log and stored only in a temporary
file that is deleted on exit. Annotations/summary never copy panic payloads.
"""
import argparse
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import os

ROOT = Path(__file__).resolve().parents[2]
IDENTIFIER = r"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*"
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
CATEGORIES = ((r"\bInvalidInput\b", "ErrInvalidInput"), (r"\bConflict\b", "Conflict"), (r"(?i)timed? out|timeout|elapsed", "timeout"), (r"\bassertion\b", "assertion"))


def location(reported, root):
    """Keep only an existing .rs file inside this checkout, never a host path."""
    reported = reported.replace("\\", "/")
    prefix = str(root.resolve()).replace("\\", "/").rstrip("/") + "/"
    if reported.lower().startswith(prefix.lower()):
        reported = reported[len(prefix):]
    if not re.fullmatch(r"(?:[A-Za-z0-9_.-]+/)*[A-Za-z0-9_.-]+\.rs", reported):
        return None
    candidate = (root / reported).resolve()
    try:
        relative = candidate.relative_to(root.resolve())
    except ValueError:
        return None
    return relative.as_posix() if candidate.is_file() else None


def source_identifiers(root):
    # A panic payload can forge harness-looking lines. Only names present in
    # this checkout's test declarations/modules may reach public annotations.
    tests, modules = set(), set()
    paths = list((root / "crates").rglob("*.rs")) + list((root / "apps/desktop/src-tauri/src").rglob("*.rs"))
    for path in paths:
        try:
            path.resolve().relative_to(root.resolve())
        except ValueError:
            continue
        source = path.read_text(encoding="utf-8", errors="replace")
        modules.add(path.stem)
        modules.update(re.findall(r"\bmod\s+([A-Za-z_][A-Za-z0-9_]*)", source))
        tests.update(re.findall(r"#\[(?:(?:tokio|async_std)::)?test(?:\([^\]]*\))?\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)", source))
    return tests, modules


def parse_failures(text, root=ROOT):
    tests, modules = source_identifiers(root)
    failures = {}
    current = None
    for line in text.splitlines():
        line = ANSI.sub("", line)
        failed = re.fullmatch(r"test (" + IDENTIFIER + r") \.\.\. FAILED\s*", line)
        group = re.fullmatch(r"---- (" + IDENTIFIER + r") stdout ----", line)
        panic = re.match(r"^thread '(" + IDENTIFIER + r")' panicked at (.+):(\d+):(\d+):$", line)
        name = (panic or group or failed)
        if name:
            current = name.group(1)
            segments = current.split("::")
            if len(current) > 256 or segments[-1] not in tests or any(segment not in modules for segment in segments[:-1]):
                current = None
                continue
            failures.setdefault(current, {"test": current, "category": "test_failure"})
        if panic and current:
            file = location(panic.group(2), root)
            row, column = int(panic.group(3)), int(panic.group(4))
            if file and 0 < row <= 10000000 and 0 < column <= 100000:
                failures[current].update(file=file, line=row, col=column)
        # assert_eq's expected/actual debug lines can contain error enum names;
        # they must not replace the already identified assertion failure class.
        if current and failures[current]["category"] != "assertion":
            for pattern, category in CATEGORIES:
                if re.search(pattern, line):
                    failures[current]["category"] = category
                    break
        compile_error = re.match(r"^error\[(E\d{4})\]:", line)
        if compile_error:
            key = "compile_" + compile_error.group(1)
            failures[key] = {"test": key, "category": "compilecode"}
    return [failures[key] for key in sorted(failures)][:50]


def escape(value, property=False):
    value = str(value).replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    return value.replace(",", "%2C").replace(":", "%3A") if property else value


def report(failures, exit_code, summary_path=None):
    if not exit_code:
        return
    if not failures:
        failures = [{"test": "unclassified_failure", "category": "test_failure"}]
    for failure in failures[:10]:
        properties = ["title=Rust core test failure"]
        for key in ("file", "line", "col"):
            if key in failure:
                properties.append(key + "=" + escape(failure[key], property=True))
        print("::error " + ",".join(properties) + "::" + escape(failure["test"] + " [" + failure["category"] + "]"))
    if summary_path:
        with open(summary_path, "a", encoding="utf-8") as handle:
            handle.write("## Rust core test failures\n\nCargo exit status: " + str(exit_code) + "\n\n")
            handle.write("| Test identifier | Location in repository | Category |\n| --- | --- | --- |\n")
            for failure in failures:
                source = failure.get("file", "")
                if source:
                    source += ":" + str(failure["line"]) + ":" + str(failure["col"])
                handle.write("| `" + failure["test"] + "` | `" + source + "` | " + failure["category"] + " |\n")
            handle.write("\nOnly allowlisted identifiers, repository locations and fixed categories are shown. Panic payloads and host paths are omitted.\n")


def safe_report(log, exit_code, root, summary_path):
    try:
        if log is None:
            failures = []
        else:
            with log.open("rb") as handle:
                handle.seek(max(0, log.stat().st_size - 8 * 1024 * 1024))
                failures = parse_failures(handle.read().decode("utf-8", errors="replace"), root)
        report(failures, exit_code, summary_path)
    except (OSError, ValueError, RuntimeError, UnicodeError):
        # Optional metadata/summary failure must never replace Cargo's status.
        try:
            print("::error title=Rust core test failure::unclassified_failure [test_failure]; diagnostic metadata unavailable")
        except OSError:
            pass


def run(command, root=ROOT, summary_path=None):
    with tempfile.TemporaryDirectory(prefix="mediaatlas-cargo-") as temporary:
        log = Path(temporary) / "cargo.log"
        with log.open("wb") as handle:
            try:
                process = subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            except OSError:
                safe_report(None, 127, root, summary_path)
                return 127
            for chunk in iter(lambda: process.stdout.read(4096), b""):
                handle.write(chunk)
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
            exit_code = process.wait()
        if exit_code:
            safe_report(log, exit_code, root, summary_path)
        return exit_code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("expected Cargo command after --")
    status = run(command, summary_path=os.environ.get("GITHUB_STEP_SUMMARY"))
    sys.exit(status if status >= 0 else 128 - status)


if __name__ == "__main__":
    main()
