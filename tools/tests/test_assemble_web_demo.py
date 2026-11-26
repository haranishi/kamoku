# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""お試し版の組み立て（assemble-web-demo.py）の、消してよい出力先・ライセンスの起点・配り漏れの検査。"""

import importlib.util
import io
from contextlib import redirect_stderr
from pathlib import Path
import shutil
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("assemble_web_demo", ROOT / "tools/assemble-web-demo.py")
ASSEMBLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ASSEMBLE)


def stops(function, *args):
    """fail()（SystemExit）で止まったら、その日本語の理由を返す。止まらなければ None。"""
    err = io.StringIO()
    try:
        with redirect_stderr(err):
            function(*args)
    except SystemExit:
        return err.getvalue()
    return None


class ClearOutputTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="kamoku-assemble-")
        self.addCleanup(self.temp.cleanup)
        self.out = Path(self.temp.name) / "out"

    def test_missing_and_empty_directories_are_fine(self):
        self.assertIsNone(stops(ASSEMBLE.clear_output, self.out))
        self.out.mkdir()
        self.assertIsNone(stops(ASSEMBLE.clear_output, self.out))
        self.assertFalse(self.out.exists())

    def test_previous_output_is_replaced(self):
        for marker in ASSEMBLE.OUTPUT_MARKERS:
            (self.out / marker).parent.mkdir(parents=True, exist_ok=True)
            (self.out / marker).write_text("x")
        (self.out / "css").mkdir()
        (self.out / "css/base.css").write_text("x")
        self.assertIsNone(stops(ASSEMBLE.clear_output, self.out))
        self.assertFalse(self.out.exists())

    def test_other_files_are_kept_and_it_stops(self):
        self.out.mkdir()
        (self.out / "家計簿メモ.txt").write_text("消えてはいけない")
        (self.out / "build-info.json").write_text("{}")
        reason = stops(ASSEMBLE.clear_output, self.out)
        self.assertIsNotNone(reason)
        self.assertIn("何も消さずに止めました", reason)
        self.assertEqual((self.out / "家計簿メモ.txt").read_text(), "消えてはいけない")
        self.assertTrue((self.out / "build-info.json").exists())

    def test_previous_output_with_other_files_is_kept(self):
        # 取り込み先のサイトのフォルダのように、出力の印があっても、ほかの物があれば消さない。
        for marker in ASSEMBLE.OUTPUT_MARKERS:
            (self.out / marker).parent.mkdir(parents=True, exist_ok=True)
            (self.out / marker).write_text("x")
        (self.out / "meta.json").write_text("消えてはいけない")
        reason = stops(ASSEMBLE.clear_output, self.out)
        self.assertIsNotNone(reason)
        self.assertIn("何も消さずに止めました", reason)
        self.assertEqual((self.out / "meta.json").read_text(), "消えてはいけない")
        self.assertTrue((self.out / "web/shim.js").exists())

    def test_a_file_in_place_of_the_directory_is_kept(self):
        self.out.write_text("keep")
        self.assertIsNotNone(stops(ASSEMBLE.clear_output, self.out))
        self.assertEqual(self.out.read_text(), "keep")


class UiPkgTest(unittest.TestCase):
    def test_extra_generated_files_stop_the_build(self):
        with tempfile.TemporaryDirectory(prefix="kamoku-ui-pkg-") as name:
            pkg = Path(name)
            for file in ASSEMBLE.UI_PKG_FILES:
                (pkg / file).write_text("x")
            self.assertIsNone(stops(ASSEMBLE.check_ui_pkg, pkg))
            (pkg / "snippets").mkdir()
            reason = stops(ASSEMBLE.check_ui_pkg, pkg)
            self.assertIsNotNone(reason)
            self.assertIn("snippets", reason)


class BuildInfoTest(unittest.TestCase):
    def test_modified_flag_only_when_dirty(self):
        self.assertEqual(ASSEMBLE.build_info("abc", False), {"commit": "abc"})
        self.assertEqual(ASSEMBLE.build_info("abc", True), {"commit": "abc", "modified": True})


@unittest.skipUnless(shutil.which("cargo"), "cargo が無い")
class LicensesTest(unittest.TestCase):
    def test_both_wasm_roots_are_listed(self):
        packages = ASSEMBLE.wasm_packages()
        names = {pkg["name"] for pkg in packages}
        for name in (
            "kamoku-web", "kamoku-ui", "sqlite-wasm-rs", "unicode-normalization",
            "num-bigint", "num-traits", "web-sys", "wasm-bindgen-futures",
        ):
            self.assertIn(name, names)
        text = ASSEMBLE.license_text(packages)
        self.assertIn("Rust の標準ライブラリ（MIT OR Apache-2.0）", text)
        self.assertIn("## web-sys ", text)
        section = text.split("## rsqlite-vfs ", 1)[1].split("\n## ", 1)[0]
        self.assertIn("未確認", section)
        self.assertIn("Permission is hereby granted", section)


if __name__ == "__main__":
    unittest.main()
