import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("rust_tests", Path(__file__).with_name("rust_tests.py"))
helper = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(helper)


class SafeSummaryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        source = self.root / "crates/core/src/tests.rs"
        source.parent.mkdir(parents=True)
        source.write_text("mod module { #[test] fn case() {} } mod safe { #[test] fn case() {} }\n")

    def test_panic_has_only_identifier_location_and_fixed_category(self):
        log = "test module::case ... FAILED\n---- module::case stdout ----\nthread 'module::case' panicked at crates/core/src/tests.rs:19:7:\ncalled Result::unwrap() on an Err value: InvalidInput SECRET-PAYLOAD\n"
        result = helper.parse_failures(log, self.root)
        self.assertEqual(result, [{"test": "module::case", "category": "ErrInvalidInput", "file": "crates/core/src/tests.rs", "line": 19, "col": 7}])

    def test_windows_separators_absolute_checkout_and_external_paths(self):
        absolute = str(self.root / "crates/core/src/tests.rs").replace("/", "\\")
        self.assertEqual(helper.location(absolute, self.root), "crates/core/src/tests.rs")
        for path in ("C:\\Users\\secret-user\\private.rs", "/home/secret/private.rs", "../secret.rs", "crates/../../secret.rs"):
            self.assertIsNone(helper.location(path, self.root))

    def test_control_newline_payload_cannot_create_annotations(self):
        log = "---- safe::case stdout ----\nthread 'safe::case' panicked at /home/SECRET/private.rs:2:1:\nassertion failed: TOKEN=SECRET\n::error file=/home/SECRET::LEAK\n---- SECRET_TOKEN::case stdout ----\nthread 'SECRET_TOKEN::case' panicked at crates/core/src/tests.rs:1:1:\n"
        failures = helper.parse_failures(log, self.root)
        output = io.StringIO()
        summary = self.root / "summary.md"
        with contextlib.redirect_stdout(output):
            helper.report(failures, 101, summary)
        combined = output.getvalue() + summary.read_text()
        self.assertNotIn("SECRET", combined)
        self.assertNotIn("LEAK", combined)
        self.assertNotIn("/home/", combined)
        self.assertEqual(output.getvalue().count("::error"), 1)
        self.assertIn("assertion", combined)

    def test_compile_error_code_and_fixed_fallback_do_not_copy_messages(self):
        failures = helper.parse_failures("error[E0432]: secret-user/token/path\n", self.root)
        self.assertEqual(failures, [{"test": "compile_E0432", "category": "compilecode"}])
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            helper.report([], 101)
        self.assertIn("unclassified_failure [test_failure]", output.getvalue())

    def test_categories_and_annotation_limits(self):
        mismatch = helper.parse_failures("---- safe::case stdout ----\nassertion `left == right` failed\n  left: Ok(())\n right: Err(Conflict)\n", self.root)
        self.assertEqual(mismatch[0]["category"], "assertion")
        for message, category in (("Conflict", "Conflict"), ("operation timed out", "timeout")):
            failures = helper.parse_failures("---- safe::case stdout ----\n" + message, self.root)
            self.assertEqual(failures[0]["category"], category)
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            helper.report([{"test": "safe_" + str(i), "category": "assertion"} for i in range(50)], 101)
        self.assertEqual(output.getvalue().count("::error"), 10)

    def test_summary_io_failure_cannot_replace_original_exit_status(self):
        result = subprocess.run([sys.executable, str(Path(helper.__file__)), "--", sys.executable, "-c", "raise SystemExit(101)"], env={**os.environ, "GITHUB_STEP_SUMMARY": str(self.root)}, capture_output=True, text=True)
        self.assertEqual(result.returncode, 101)
        self.assertIn("diagnostic metadata unavailable", result.stdout)
        self.assertNotIn(str(self.root), result.stdout + result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_wrapper_preserves_success_and_cargo_style_failure_status(self):
        for status in (0, 101):
            summary = self.root / ("summary-" + str(status) + ".md")
            result = subprocess.run([sys.executable, str(Path(helper.__file__)), "--", sys.executable, "-c", "raise SystemExit(" + str(status) + ")"], env={**os.environ, "GITHUB_STEP_SUMMARY": str(summary)}, capture_output=True, text=True)
            self.assertEqual(result.returncode, status)
            self.assertEqual(summary.exists(), bool(status))


if __name__ == "__main__":
    unittest.main()
