# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""お試し版の出力の検査（手元の絶対パス・秘密に見える行・大きさ）を、小さな出力で確かめる。"""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools/check-web-demo-output.py"
SPEC = importlib.util.spec_from_file_location("check_web_demo_output", SCRIPT)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class WebDemoOutputTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="kamoku-web-demo-out-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "web").mkdir()
        (self.root / "index.html").write_text("<!doctype html><title>kamoku</title>\n")
        (self.root / "web/shim.js").write_text("const token = null;\n")

    def run_script(self):
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(self.root)],
            capture_output=True, text=True, env={"HOME": "/Users/someone"},
        )

    def test_clean_output_passes_and_lists_sizes(self):
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("index.html", done.stdout)
        self.assertIn("web/shim.js", done.stdout)

    def test_absolute_paths_in_binary_fail(self):
        for needle in (b"/Users/", b"/home/", b"/Volumes/"):
            with self.subTest(needle=needle):
                (self.root / "a.wasm").write_bytes(b"\x00asm" + needle + b"x/src/lib.rs\x00")
                done = self.run_script()
                self.assertEqual(done.returncode, 1)
                self.assertIn("a.wasm", done.stderr)

    def test_home_value_fails(self):
        found = CHECK.problems(self.root, "/opt/builder")
        self.assertEqual(found, [])
        (self.root / "b.js").write_text("// /opt/builder/work/x.rs\n")
        self.assertTrue(CHECK.problems(self.root, "/opt/builder"))
        # 「/」だけの HOME では、すべてに当たるので使わない。
        self.assertEqual(CHECK.markers("/"), list(CHECK.PATH_MARKERS))

    def test_secret_like_lines_fail(self):
        # 秘密情報の走査（gitleaks）が本物の鍵と読まないように、行をつなげて作る。
        (self.root / "web/shim.js").write_text("const api" + 'Key = "' + "abcdefgh12345" + '";\n')
        done = self.run_script()
        self.assertEqual(done.returncode, 1)
        self.assertIn("web/shim.js:1", done.stderr)
        (self.root / "web/shim.js").write_text("const password = 'short';\n")
        self.assertEqual(self.run_script().returncode, 0)

    def test_files_over_25_mib_fail(self):
        big = self.root / "big.wasm"
        with big.open("wb") as handle:
            handle.truncate(CHECK.MAX_BYTES + 1)
        done = self.run_script()
        self.assertEqual(done.returncode, 1)
        self.assertIn("25MiB", done.stderr)


if __name__ == "__main__":
    unittest.main()
