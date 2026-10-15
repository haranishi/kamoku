# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

import json
import subprocess
import sys

from workflow_fixture import WorkflowFixture


class RunRealTests(WorkflowFixture):
    def run_real(self, *arguments, **environment):
        return self.run_script("e2e/run-real.sh", *arguments, **environment)

    def test_help_and_invalid_modes_do_not_start_anything(self):
        for arguments, expected in [
            (["--help"], 0), (["flow"], 2), (["--unknown"], 2), ([""], 2),
            (["all", "extra"], 2), (["contract", "extra"], 2), (["--help", "extra"], 2),
        ]:
            with self.subTest(arguments=arguments):
                result = self.run_real(*arguments)
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertEqual(self.events(), [])

    def test_missing_tools_fail_before_build_and_server_start(self):
        for tool in ("node", "cargo", "python3"):
            with self.subTest(tool=tool):
                path = self.bin / tool
                path.unlink()
                try:
                    result = self.run_real("all")
                    self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                    self.assertIn(tool, result.stderr)
                    self.assert_no_checks()
                finally:
                    path.symlink_to(self.dispatcher)

    def test_old_node_fails_before_build(self):
        result = self.run_real("contract", KAMOKU_TEST_NODE_VERSION="v22.0.0")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assert_no_checks()

    def test_playwright_must_be_installed_locally(self):
        self.cli.unlink()
        result = self.run_real("flows")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("npm ci", result.stderr)
        self.assert_no_checks()

    def test_unloadable_local_playwright_fails_before_build(self):
        result = self.run_real("flows", KAMOKU_TEST_FAILURES=json.dumps({"node:-e": 2}))
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assert_no_checks()

    def test_contract_does_not_require_playwright(self):
        self.cli.unlink()
        result = self.run_real("contract")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue(any(event["args"] == ["--test", "contract.test.mjs"]
                            for event in self.commands("node")))

    def test_build_failure_does_not_fall_back_to_an_old_binary(self):
        result = self.run_real("all", KAMOKU_TEST_FAILURES=json.dumps({"cargo:build": 7}))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("error: synthetic Rust compiler failure", result.stderr)
        self.assertEqual(self.commands("kamoku"), [])
        self.assertEqual(self.commands("python3"), [])

    def test_bad_artifacts_fail_before_starting_a_server(self):
        for mode in ("missing", "invalid", "ambiguous"):
            with self.subTest(mode=mode):
                result = self.run_real("contract", KAMOKU_TEST_ARTIFACT_MODE=mode)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(self.commands("kamoku"), [])

    def test_artifact_must_be_an_existing_executable(self):
        self.artifact.chmod(0o644)
        result = self.run_real("contract")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(self.commands("kamoku"), [])
        self.artifact.unlink()
        result = self.run_real("contract")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(self.commands("kamoku"), [])

    def test_build_uses_the_reported_artifact_with_a_custom_target_and_spaces(self):
        result = self.run_real("contract", CARGO_TARGET_DIR=str(self.artifact.parent.parent))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.commands("cargo")[0]["args"],
                         ["build", "-p", "kamoku-app", "--bin", "kamoku", "--locked", "--message-format=json-render-diagnostics"])
        self.assertEqual(self.commands("kamoku")[0]["executable"], str(self.artifact))
        self.assertIn("再ビルド（既定）", result.stdout)

    def test_explicit_binary_does_not_build_or_require_cargo_or_python_for_contract(self):
        (self.bin / "cargo").unlink()
        (self.bin / "python3").unlink()
        result = self.run_real("contract", KAMOKU_BIN=str(self.artifact))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.commands("cargo") + self.commands("python3"), [])
        self.assertEqual(self.commands("kamoku")[0]["executable"], str(self.artifact))
        self.assertIn("KAMOKU_BINの指定（ビルドを省略）", result.stdout)

    def test_invalid_explicit_binary_is_rejected(self):
        for binary in ("", str(self.root / "missing"), str(self.artifact.parent)):
            with self.subTest(binary=binary):
                result = self.run_real("contract", KAMOKU_BIN=binary)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assert_no_checks()

    def test_relative_explicit_binary_uses_the_file_checked_in_the_e2e_directory(self):
        binary = self.root / "e2e/kamoku"
        self.make_binary(binary)
        result = self.run_real("contract", KAMOKU_BIN="kamoku")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.commands("kamoku")[0]["executable"], str(binary))
        self.assertEqual(self.commands("cargo"), [])

    def test_extra_arguments_reach_only_the_selected_stage(self):
        for mode in ("flows", "screens", "docs-images"):
            with self.subTest(mode=mode):
                result = self.run_real(mode, "file with spaces.mjs", "-g", "AC-11.6",
                                       KAMOKU_BIN=str(self.artifact))
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                last = self.commands("node")[-1]["args"]
                prefix = (["docs-images.mjs"] if mode == "docs-images" else
                          ["./node_modules/@playwright/test/cli.js", "test", "--project=" + mode])
                self.assertEqual(last, prefix + ["file with spaces.mjs", "-g", "AC-11.6"])
                self.assertEqual(self.commands("npm") + self.commands("npx"), [])

    def test_configured_browser_channel_does_not_require_default_chromium(self):
        result = self.run_real("flows", KAMOKU_BIN=str(self.artifact),
                               KAMOKU_E2E_CHANNEL="chrome", KAMOKU_TEST_NO_CHROMIUM="1")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.commands("node")[-1]["channel"], "chrome")

    def test_smoke_failure_prevents_all_browser_and_contract_stages(self):
        result = self.run_real("all", KAMOKU_TEST_FAILURES=json.dumps({"python3:../tools/smoke-demo.py": 9}))
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.commands("kamoku"), [])
        self.assertEqual(self.commands("python3")[-1]["args"],
                         ["../tools/smoke-demo.py", "--bin", str(self.artifact)])
        self.assertFalse(any("--project=flows" in event["args"] for event in self.commands("node")))

    def test_all_runs_smoke_and_each_stage_with_separate_server_lifetimes(self):
        result = self.run_real("all")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        events = self.events()
        smoke = next(i for i, event in enumerate(events) if "../tools/smoke-demo.py" in event["args"])
        stages = [(i, event) for i, event in enumerate(events) if event["tool"] == "node"
                  and ("test" in event["args"] or "--test" in event["args"])]
        self.assertLess(smoke, stages[0][0])
        self.assertEqual([event["args"][-1] for _, event in stages],
                         ["--project=flows", "contract.test.mjs", "--project=screens"])
        active = set()
        previous_stage_finished = False
        for event in events:
            if event["event"] == "server-start":
                if previous_stage_finished:
                    self.assertFalse(active, "a previous stage's server was still running")
                    previous_stage_finished = False
                active.add(event["pid"])
            elif event["event"] == "server-stop":
                active.remove(event["pid"])
            elif event["tool"] == "node" and ("test" in event["args"] or "--test" in event["args"]):
                previous_stage_finished = True
        self.assertEqual(active, set())
        servers = self.commands("kamoku")
        self.assertEqual(len(servers), 5)
        data_paths = [event["args"][2] for event in servers if event["args"][0] == "serve"]
        self.assertEqual(len(set(data_paths)), 2)

    def test_early_server_exit_fails_promptly_without_private_logs(self):
        result = self.run_real("contract", KAMOKU_BIN=str(self.artifact), KAMOKU_TEST_SERVER_DIES="1")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("起動前に終了", result.stderr)
        self.assertNotIn("private startup diagnostic", result.stdout + result.stderr)
        self.assertNotIn("http://localhost", result.stdout + result.stderr)
        self.assertFalse(any(event["args"] == ["--test", "contract.test.mjs"]
                             for event in self.commands("node")))

    def test_failed_browser_stage_still_stops_its_servers(self):
        result = self.run_real("flows", KAMOKU_BIN=str(self.artifact),
                               KAMOKU_TEST_FAILURES=json.dumps({"node:./node_modules/@playwright/test/cli.js": 7}))
        self.assertEqual(result.returncode, 7, result.stdout + result.stderr)
        started = {event["pid"] for event in self.events() if event["event"] == "server-start"}
        stopped = {event["pid"] for event in self.events() if event["event"] == "server-stop"}
        self.assertEqual(len(started), 2)
        self.assertEqual(started, stopped)


class CargoExecutableTests(WorkflowFixture):
    def extract(self, content):
        cargo_output = self.root / "build.jsonl"
        cargo_output.write_text(content, encoding="utf-8")
        return subprocess.run([sys.executable, str(self.root / "tools/cargo-executable.py"), str(cargo_output)],
                              capture_output=True, text=True, timeout=5)

    def test_ignores_dependencies_and_libraries_and_deduplicates_the_executable(self):
        messages = [
            {"reason": "compiler-artifact", "target": {"name": "dependency", "kind": ["bin"]},
             "executable": "/wrong/dependency"},
            {"reason": "compiler-artifact", "target": {"name": "kamoku", "kind": ["lib"]},
             "executable": "/wrong/library"},
            {"reason": "compiler-artifact", "target": {"name": "kamoku", "kind": ["bin"]},
             "executable": str(self.artifact)},
        ]
        result = self.extract("\n".join(json.dumps(message) for message in messages + [messages[-1]]))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), str(self.artifact))

    def test_malformed_or_missing_artifact_is_an_error(self):
        for content in ("", "not JSON", "[]", "null", '{"reason": "build-finished", "success": true}'):
            with self.subTest(content=content):
                result = self.extract(content)
                self.assertEqual(result.returncode, 1, result.stderr)
