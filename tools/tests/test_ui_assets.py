# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Exercise the actual asset build script without building the application."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
ASSETS = ("kamoku_ui.js", "kamoku_ui_bg.wasm")


class UiAssetsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tool_dir = tempfile.TemporaryDirectory(prefix="kamoku-asset-build-")
        cls.binary = Path(cls.tool_dir.name) / "asset-build"
        subprocess.run(
            ["rustc", "--edition=2021", str(ROOT / "crates/kamoku-app/build.rs"),
             "-o", str(cls.binary)], check=True, capture_output=True, text=True,
        )

    @classmethod
    def tearDownClass(cls):
        cls.tool_dir.cleanup()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="kamoku-asset-fixture-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = self.root / "crates/kamoku-app"
        self.manifest.mkdir(parents=True)
        self.pkg = self.root / "ui/rust/pkg"
        self.pkg.mkdir(parents=True)
        for name in ASSETS:
            (self.pkg / name).write_bytes(b"synthetic build fixture")
        self.out = self.root / "out"
        self.out.mkdir()

    def run_build(self):
        env = dict(os.environ, CARGO_MANIFEST_DIR=str(self.manifest), OUT_DIR=str(self.out))
        return subprocess.run([str(self.binary)], env=env, capture_output=True, text=True)

    def assert_rejected(self):
        result = self.run_build()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Run tools/build-rust-ui.sh", result.stderr)
        self.assertFalse((self.out / "ui_files.rs").exists())

    def test_regular_assets_are_both_embedded(self):
        result = self.run_build()
        self.assertEqual(result.returncode, 0, result.stderr)
        generated = (self.out / "ui_files.rs").read_text()
        for name in ASSETS:
            self.assertIn(f'"rust/pkg/{name}"', generated)

    def test_each_missing_asset_fails_before_writing_manifest(self):
        for name in ASSETS:
            with self.subTest(name=name):
                target = self.pkg / name
                target.unlink()
                self.assert_rejected()
                target.write_bytes(b"synthetic build fixture")

    @unittest.skipUnless(hasattr(os, "symlink"), "requires symlinks")
    def test_symlink_asset_is_not_mistaken_for_embedded_file(self):
        target = self.pkg / ASSETS[0]
        target.unlink()
        outside = self.root / "outside.js"
        outside.write_bytes(b"synthetic build fixture")
        target.symlink_to(outside)
        self.assert_rejected()

    @unittest.skipUnless(hasattr(os, "symlink"), "requires symlinks")
    def test_symlink_package_directory_is_not_mistaken_for_embedded_assets(self):
        outside = self.root / "outside-pkg"
        shutil.move(str(self.pkg), outside)
        self.pkg.symlink_to(outside, target_is_directory=True)
        self.assert_rejected()
