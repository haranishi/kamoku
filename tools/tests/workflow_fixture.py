# SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Isolated command stubs; no builds, downloads, or changes to the caller's HOME."""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
STUB = r'''
import json
import os
import signal
import sys
import time
from pathlib import Path

tool = Path(sys.argv[0]).name
args = sys.argv[1:]

def record(event, **detail):
    data = {"event": event, "tool": tool, "args": args, "pid": os.getpid(), **detail}
    fd = os.open(os.environ["KAMOKU_TEST_LOG"], os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
    try:
        os.write(fd, (json.dumps(data) + "\n").encode())
    finally:
        os.close(fd)

record("command", executable=sys.argv[0], channel=os.environ.get("KAMOKU_E2E_CHANNEL"))
failures = json.loads(os.environ.get("KAMOKU_TEST_FAILURES", "{}"))
failure = failures.get(tool + ":" + (args[0] if args else ""))
package = args[args.index("-p") + 1] if tool == "cargo" and "-p" in args else None
if tool == "cargo" and args and args[0] == "build":
    failure = failures.get("cargo:build:" + str(package), failure)
if tool == "wasm-bindgen" and args != ["--version"]:
    failure = failures.get("wasm-bindgen:generate", failure)
if failure:
    if tool == "cargo" and args and args[0] == "build":
        diagnostic = "error: synthetic Rust compiler failure"
        if "--message-format=json-render-diagnostics" in args or not any(a.startswith("--message-format=") for a in args):
            print(diagnostic, file=sys.stderr)
        else:
            print(json.dumps({"reason": "compiler-message", "message": {"rendered": diagnostic}}))
    sys.exit(failure)

if tool == "node":
    if args == ["--version"]:
        print(os.environ.get("KAMOKU_TEST_NODE_VERSION", "v24.12.0"))
    elif args and args[0] == "-e":
        if os.environ.get("KAMOKU_TEST_NO_CHROMIUM") and "executablePath" in args[1]:
            sys.exit(2)
elif tool == "rustup":
    if args == ["toolchain", "list"]:
        print(os.environ.get("KAMOKU_TEST_TOOLCHAINS", "1.88.0-test-host\n1.99.0-test-host (default)"))
    elif args[:3] == ["target", "list", "--installed"] and (len(args) == 3 or args[3:4] == ["--toolchain"]):
        key = "KAMOKU_TEST_MSRV_TARGETS" if len(args) > 3 else "KAMOKU_TEST_TARGETS"
        print(os.environ.get(key, "test-host\nwasm32-unknown-unknown"))
    else:
        sys.exit(99)
elif tool == "wasm-bindgen":
    if args == ["--version"]:
        print(os.environ.get("KAMOKU_TEST_WASM_BINDGEN_VERSION", "wasm-bindgen 0.2.129"))
    else:
        source = Path(args[0])
        if not source.is_file():
            sys.exit(8)
        output = Path(args[args.index("--out-dir") + 1])
        output.mkdir(parents=True, exist_ok=True)
        (output / "kamoku_ui.js").write_text("// synthetic generated WASM loader\n")
        (output / "kamoku_ui_bg.wasm").write_bytes(source.read_bytes())
elif tool == "cargo" and args and args[0] == "metadata":
    target = os.environ.get("CARGO_TARGET_DIR", str(Path(os.environ["KAMOKU_TEST_ARTIFACT"]).parents[1]))
    print(json.dumps({"target_directory": str(Path(target).resolve())}))
elif tool == "cargo" and args and args[0] == "build":
    if package == "kamoku-ui":
        target = os.environ.get("CARGO_TARGET_DIR", str(Path(os.environ["KAMOKU_TEST_ARTIFACT"]).parents[1]))
        wasm = Path(target) / "wasm32-unknown-unknown/release/kamoku_ui.wasm"
        wasm.parent.mkdir(parents=True, exist_ok=True)
        wasm.write_bytes(b"\0asm\1\0\0\0")
    else:
        mode = os.environ.get("KAMOKU_TEST_ARTIFACT_MODE", "normal")
        if mode == "invalid":
            print("invalid JSON")
        elif mode != "missing":
            artifact = {"reason": "compiler-artifact", "target": {"name": "kamoku", "kind": ["bin"]},
                        "executable": os.environ["KAMOKU_TEST_ARTIFACT"]}
            print(json.dumps(artifact))
            if mode == "ambiguous":
                artifact["executable"] += "-other"
                print(json.dumps(artifact))
elif tool == "python3" and args and (args[0] == "-c" or args[0].endswith("cargo-executable.py")):
    os.execv(sys.executable, [sys.executable, *args])
elif tool == "kamoku":
    if os.environ.get("KAMOKU_TEST_SERVER_DIES"):
        print("private startup diagnostic: http://localhost:4321/?t=" + "a" * 32, flush=True)
        sys.exit(9)
    def stopped(signum, frame):
        record("server-stop")
        sys.exit(0)
    signal.signal(signal.SIGTERM, stopped)
    record("server-start", executable=sys.argv[0])
    print("http://127.0.0.1:4321/?t=" + "b" * 32, flush=True)
    while True:
        time.sleep(0.05)
elif tool in {"npm", "npx"}:
    sys.exit(99)
'''


class WorkflowFixture(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="kamoku-workflow-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.bin = self.root / "commands"
        self.bin.mkdir()
        self.log = self.root / "commands.jsonl"
        self.dispatcher = self.root / "command-stub"
        self.dispatcher.write_text(f"#!{sys.executable}\n" + STUB, encoding="utf-8")
        self.dispatcher.chmod(0o755)
        for tool in ("cargo", "rustup", "cargo-deny", "wasm-bindgen", "node", "python3", "uvx", "git", "npm", "npx"):
            (self.bin / tool).symlink_to(self.dispatcher)
        for tool in ("bash", "dirname", "env", "grep", "head", "mktemp", "rm", "sleep", "mkdir"):
            (self.bin / tool).symlink_to(shutil.which(tool))
        # Copy the production runners unchanged, with a synthetic repository around them.
        for relative in ("tools/check-all.sh", "tools/build-rust-ui.sh", "tools/cargo-executable.py", "e2e/run-real.sh"):
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, destination)
        self.cli = self.root / "e2e/node_modules/@playwright/test/cli.js"
        self.cli.parent.mkdir(parents=True)
        self.cli.touch()
        self.artifact = self.root / "custom target with spaces/debug/kamoku"
        self.make_binary(self.artifact)
        self.make_binary(self.root / "target/debug/kamoku")
        self.environment = os.environ.copy()
        self.environment.pop("KAMOKU_BIN", None)
        self.environment.update({
            "PATH": str(self.bin),
            "KAMOKU_TEST_LOG": str(self.log),
            "KAMOKU_TEST_ARTIFACT": str(self.artifact),
        })

    def make_binary(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(self.dispatcher, path)

    def run_script(self, script, *arguments, **environment):
        self.log.unlink(missing_ok=True)
        return subprocess.run(
            ["/bin/bash", str(self.root / script), *arguments],
            cwd=self.root, env={**self.environment, **environment},
            capture_output=True, text=True, timeout=10,
        )

    def events(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def commands(self, tool):
        return [event for event in self.events() if event["event"] == "command" and event["tool"] == tool]

    def assert_no_checks(self):
        for tool in ("cargo", "python3", "uvx", "kamoku", "npm", "npx"):
            self.assertEqual(self.commands(tool), [], tool)
        self.assertEqual([event for event in self.commands("wasm-bindgen") if event["args"] != ["--version"]], [])

    def assert_no_tool_installs(self):
        self.assertEqual(self.commands("npm") + self.commands("npx"), [])
        self.assertFalse(any(event["args"][:2] in (["target", "add"], ["toolchain", "install"])
                             for event in self.commands("rustup")))
        self.assertFalse(any(event["args"] and event["args"][0] == "install" for event in self.commands("cargo")))
