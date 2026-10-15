# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

import json

from workflow_fixture import WorkflowFixture


class CheckAllTests(WorkflowFixture):
    def run_check(self, *arguments, **environment):
        return self.run_script("tools/check-all.sh", *arguments, **environment)

    def test_help_and_invalid_arguments_do_not_start_checks(self):
        for arguments, expected in [
            (["--help"], 0), (["--quik"], 2), (["unexpected"], 2), ([""], 2),
            (["--quick", "extra"], 2), (["--help", "extra"], 2),
        ]:
            with self.subTest(arguments=arguments):
                result = self.run_check(*arguments)
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertEqual(self.events(), [])

    def test_every_missing_required_command_fails_before_checks(self):
        for tool in ("cargo", "node", "python3", "uvx", "git", "rustup", "cargo-deny"):
            with self.subTest(tool=tool):
                path = self.bin / tool
                path.unlink()
                try:
                    result = self.run_check()
                    self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                    self.assertIn(tool, result.stderr)
                    self.assert_no_checks()
                finally:
                    path.symlink_to(self.dispatcher)

    def test_msrv_must_be_installed_without_matching_1880(self):
        for toolchains in ("stable-test-host", "1.880-test-host", "1.88.1-test-host"):
            with self.subTest(toolchains=toolchains):
                result = self.run_check(KAMOKU_TEST_TOOLCHAINS=toolchains)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn("rustup toolchain install 1.88", result.stderr)
                self.assert_no_checks()

    def test_failed_toolchain_query_is_a_missing_prerequisite(self):
        result = self.run_check(KAMOKU_TEST_FAILURES=json.dumps({"rustup:toolchain": 1}))
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assert_no_checks()

    def test_old_or_unreadable_node_is_rejected(self):
        for version in ("v23.9.0", "invalid"):
            with self.subTest(version=version):
                result = self.run_check(KAMOKU_TEST_NODE_VERSION=version)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn("Node 24", result.stderr)
                self.assert_no_checks()

    def test_quick_only_omits_msrv_and_deny_and_labels_the_result(self):
        (self.bin / "rustup").unlink()
        (self.bin / "cargo-deny").unlink()
        result = self.run_check("--quick")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("明示的に省略", result.stdout)
        self.assertIn("quickの範囲は通過", result.stdout)
        self.assertIn("省略あり", result.stdout)
        self.assertNotIn("すべて通過", result.stdout)
        self.assertEqual(self.commands("rustup"), [])
        self.assertEqual(len(self.commands("cargo")), 4)
        self.assertTrue(self.commands("node"))
        self.assertTrue(self.commands("python3"))
        self.assertTrue(self.commands("uvx"))

    def test_quick_does_not_allow_a_missing_license_tool(self):
        (self.bin / "uvx").unlink()
        result = self.run_check("--quick")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assert_no_checks()

    def test_full_checks_are_locked_strict_and_offline(self):
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("完全モード", result.stdout)
        cargo = [event["args"] for event in self.commands("cargo")]
        self.assertIn(["fmt", "--all", "--", "--check"], cargo)
        for arguments in cargo:
            if arguments[0] != "fmt":  # rustfmt does not resolve Cargo dependencies.
                self.assertIn("--locked", arguments)
        self.assertIn(["+1.88.0-test-host", "check", "--workspace", "--locked"], cargo)
        python = [event["args"] for event in self.commands("python3")]
        self.assertIn(["tools/check-spec-trace.py", "--strict"], python)
        self.assertIn(["-m", "unittest", "discover", "-s", "tools/tests", "-v"], python)
        self.assertEqual(self.commands("uvx")[0]["args"],
                         ["--offline", "--from", "reuse[charset-normalizer]", "reuse", "lint"])
        self.assertEqual(self.commands("npm") + self.commands("npx"), [])

    def test_multiple_failures_are_reported_after_later_checks_run(self):
        failures = {"cargo:fmt": 4, "cargo:test": 7, "node:--test": 8,
                    "python3:-m": 9, "python3:tools/check-links.py": 5, "uvx:--offline": 3}
        result = self.run_check(KAMOKU_TEST_FAILURES=json.dumps(failures))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        summary = result.stdout.split("== まとめ")[1]
        for name in ("Rust: 書式", "Rust: 試験", "画面: 単体試験", "道具: 回帰試験",
                     "文書: リンクと行数", "ライセンス表示(REUSE)"):
            self.assertIn("失敗: " + name, summary)
        self.assertEqual(self.events()[-1]["tool"], "uvx")
        self.assertTrue(any("tools/check-spec-trace.py" in event["args"]
                            for event in self.commands("python3")))
