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
        for tool in ("cargo", "node", "python3", "uvx", "git", "rustup", "wasm-bindgen", "cargo-deny"):
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

    def test_wasm_bindgen_must_match_the_locked_runtime(self):
        for version in ("wasm-bindgen 0.2.128", "wasm-bindgen 0.2.129-extra", "invalid"):
            for arguments in ([], ["--quick"]):
                with self.subTest(version=version, arguments=arguments):
                    result = self.run_check(*arguments, KAMOKU_TEST_WASM_BINDGEN_VERSION=version)
                    self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                    self.assertIn("--version 0.2.129 --locked", result.stderr)
                    self.assert_no_checks()
                    self.assert_no_tool_installs()

    def test_wasm_target_is_required_in_both_full_and_quick_modes(self):
        for targets in ("test-host", "wasm32-unknown-unknown-extra", ""):
            for arguments in ([], ["--quick"]):
                with self.subTest(targets=targets, arguments=arguments):
                    result = self.run_check(*arguments, KAMOKU_TEST_TARGETS=targets)
                    self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                    self.assertIn("rustup target add wasm32-unknown-unknown", result.stderr)
                    self.assert_no_checks()
                    self.assert_no_tool_installs()

    def test_failed_wasm_target_query_is_not_silently_skipped(self):
        result = self.run_check("--quick", KAMOKU_TEST_FAILURES=json.dumps({"rustup:target": 4}))
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assert_no_checks()
        self.assert_no_tool_installs()

    def test_full_mode_requires_the_msrv_wasm_target_but_quick_does_not(self):
        result = self.run_check(KAMOKU_TEST_MSRV_TARGETS="test-host")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("--toolchain 1.88.0-test-host", result.stderr)
        self.assert_no_checks()
        result = self.run_check("--quick", KAMOKU_TEST_MSRV_TARGETS="test-host")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assert_no_tool_installs()

    def test_quick_only_omits_msrv_and_deny_and_labels_the_result(self):
        (self.bin / "cargo-deny").unlink()
        result = self.run_check("--quick")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("明示的に省略", result.stdout)
        self.assertIn("quickの範囲は通過", result.stdout)
        self.assertIn("省略あり", result.stdout)
        self.assertNotIn("すべて通過", result.stdout)
        self.assertEqual([event["args"] for event in self.commands("rustup")], [["target", "list", "--installed"]])
        self.assertFalse(any(event["args"][0].startswith("+") for event in self.commands("cargo")))
        self.assertFalse(any(event["args"][0] == "deny" for event in self.commands("cargo")))
        self.assertTrue(any(event["args"][:3] == ["build", "-p", "kamoku-ui"] for event in self.commands("cargo")))
        self.assertTrue(self.commands("node"))
        self.assertTrue(self.commands("python3"))
        self.assertTrue(self.commands("uvx"))
        self.assert_no_tool_installs()

    def test_quick_still_requires_license_and_wasm_tools(self):
        for tool in ("uvx", "rustup", "wasm-bindgen"):
            with self.subTest(tool=tool):
                path = self.bin / tool
                path.unlink()
                try:
                    result = self.run_check("--quick")
                    self.assertEqual(result.returncode, 2, result.stderr)
                    self.assert_no_checks()
                finally:
                    path.symlink_to(self.dispatcher)

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
        self.assertIn(["+1.88.0-test-host", "check", "-p", "kamoku-ui", "--target", "wasm32-unknown-unknown", "--locked"], cargo)
        self.assertIn(["clippy", "-p", "kamoku-ui", "--target", "wasm32-unknown-unknown", "--locked", "--", "-D", "warnings"], cargo)
        generation = next(i for i, event in enumerate(self.events()) if event["tool"] == "wasm-bindgen" and event["args"] != ["--version"])
        wasm_clippy = next(i for i, event in enumerate(self.events()) if event["tool"] == "cargo" and event["args"][:3] == ["clippy", "-p", "kamoku-ui"])
        native_clippy = next(i for i, event in enumerate(self.events()) if event["tool"] == "cargo" and event["args"][:2] == ["clippy", "--workspace"])
        self.assertLess(generation, wasm_clippy)
        self.assertLess(wasm_clippy, native_clippy)
        python = [event["args"] for event in self.commands("python3")]
        self.assertIn(["tools/check-spec-trace.py", "--strict"], python)
        self.assertIn(["-m", "unittest", "discover", "-s", "tools/tests", "-v"], python)
        self.assertEqual(self.commands("uvx")[0]["args"],
                         ["--offline", "--from", "reuse[charset-normalizer]", "reuse", "lint"])
        self.assert_no_tool_installs()

    def test_wasm_generation_and_clippy_failures_are_aggregated_with_later_checks(self):
        for failures, name in [
            ({"cargo:build:kamoku-ui": 7}, "Rust UI: WASM生成"),
            ({"wasm-bindgen:generate": 8}, "Rust UI: WASM生成"),
            ({"cargo:clippy": 9}, "Rust UI: WASM clippy"),
        ]:
            with self.subTest(failures=failures):
                result = self.run_check(KAMOKU_TEST_FAILURES=json.dumps(failures))
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn("失敗: " + name, result.stdout.split("== まとめ")[1])
                self.assertTrue(any(event["args"] == ["test", "--workspace", "--locked"] for event in self.commands("cargo")))
                self.assertEqual(self.events()[-1]["tool"], "uvx")

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
