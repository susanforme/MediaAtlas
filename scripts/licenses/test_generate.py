"""SPDX metadata must be supplemented with full terms from a pinned revision."""
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("license_generator", Path(__file__).with_name("generate.py"))
generator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generator)


class FullTermsTests(unittest.TestCase):
    def setUp(self):
        self.name, self.version = "@fixture/dialog", "2.0.0"
        self.key = (self.name, self.version)
        self.revision = "a" * 40
        self.declaration = {"original_path": "LICENSE.spdx", "path": "declaration", "sha256": "fixture", "origin": "fixture"}
        self.terms = {"original_path": "LICENSE_MIT", "path": "original-terms", "sha256": "fixture", "origin": "fixture"}
        self.metadata = {"license": "MIT", "repository": "https://github.com/example/project"}

    def generate(self, registry, terms):
        with patch.object(generator, "PROVENANCE", {}), \
             patch.object(generator, "npm_lock_packages", return_value={self.key: {}}), \
             patch.object(generator, "installed_npm", return_value={self.key: (Path("fixture"), self.metadata)}), \
             patch.object(generator, "local_notices", side_effect=lambda *_: [dict(self.declaration)]), \
             patch.object(generator, "fetch", return_value=json.dumps(registry).encode()), \
             patch.object(generator, "upstream_notices", return_value=terms) as upstream:
            result = generator.npm_inventory()
            return result, upstream.call_args

    def test_preserves_spdx_and_adds_original_terms_using_exact_registry_git_head(self):
        entries, called = self.generate({"gitHead": self.revision}, [self.terms])
        self.assertEqual(entries[0]["notices"], [self.declaration, self.terms])
        self.assertEqual(called.args[-1], self.revision)
        self.assertEqual(called.args[1], self.metadata["repository"])

    def test_missing_exact_revision_fails_instead_of_using_branch_head(self):
        with self.assertRaisesRegex(RuntimeError, "Cannot pin full upstream license"):
            self.generate({}, [self.terms])

    def test_upstream_spdx_only_still_fails_the_full_terms_gate(self):
        with self.assertRaisesRegex(RuntimeError, "SPDX declaration is not full license"):
            self.generate({"gitHead": self.revision}, [self.declaration])


if __name__ == "__main__":
    unittest.main()
