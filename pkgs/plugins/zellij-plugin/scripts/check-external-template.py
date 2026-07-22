#!/usr/bin/env python3
import argparse
import fcntl
import json
import os
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
PLUGIN_DIR = ROOT / "pkgs/plugins/zellij-plugin"
WASM = PLUGIN_DIR / "target/wasm32-wasip1/release/zellij-plugin-agent-threads.wasm"
DEMO_LAYOUT = PLUGIN_DIR / "demo-external.kdl"
MARKER = "EXTERNAL_TEMPLATE_ACTIVE"

parser = argparse.ArgumentParser()
parser.add_argument("--wasm", default=str(WASM), help="Plugin WASM path")
parser.add_argument("--template-file", help="Template path exactly as passed through KDL")
parser.add_argument(
    "--expect",
    default=MARKER,
    help="Text the template must render from the injected session title",
)
args = parser.parse_args()

session = f"agent-threads-template-repro-{os.getpid()}"
with tempfile.TemporaryDirectory(prefix="agent-threads-template-") as tmp_dir:
    tmp = Path(tmp_dir)
    generated_template = tmp / "main.jinja"
    generated_template.write_text(MARKER + "\n")
    wasm = Path(args.wasm).expanduser().absolute()
    template_file = args.template_file or str(generated_template)
    expected = args.expect
    layout = tmp / "layout.kdl"
    layout.write_text(
        DEMO_LAYOUT.read_text()
        .replace("file:target/wasm32-wasip1/release/zellij-plugin-agent-threads.wasm", f"file:{wasm}")
        .replace("/absolute/path/to/custom-template/main.jinja", template_file)
    )

    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execvp("zellij", ["zellij", "--session", session, "--new-session-with-layout", str(layout)])

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
    output = bytearray()
    deadline = time.time() + 10
    next_permission_attempt = time.time() + 0.5
    next_pipe_attempt = time.time() + 1
    try:
        while time.time() < deadline:
            readable, _, _ = select.select([fd], [], [], 0.1)
            if readable:
                try:
                    chunk = os.read(fd, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                output.extend(chunk)
            if expected.encode() in output:
                break
            now = time.time()
            if now >= next_permission_attempt:
                os.write(fd, b"y\r")
                next_permission_attempt = now + 0.5
            if now >= next_pipe_attempt:
                payload = json.dumps({
                    "version": 1,
                    "harness": "pi",
                    "session": "external-template-repro",
                    "cwd": str(ROOT),
                    "pane_id": "1",
                    "tab_id": 0,
                    "tab_name": "Demo",
                    "zellij_session": session,
                    "state": "running",
                    "model": "test",
                    "title": expected,
                    "current_task": "verify external template",
                    "updated_at": 0,
                })
                try:
                    subprocess.run(
                        ["zellij", "--session", session, "pipe", "--name", "zellij-agent-threads", "--", payload],
                        stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL,
                        timeout=2,
                    )
                except subprocess.TimeoutExpired:
                    pass
                next_pipe_attempt = now + 0.5
    finally:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass
        try:
            subprocess.run(
                ["zellij", "kill-session", session],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=2,
            )
        except subprocess.TimeoutExpired:
            pass

if expected.encode() not in output:
    diagnostics = [
        match.group().decode("utf-8", "replace").strip()
        for match in re.finditer(rb"[ -~]{12,}", output)
        if b"template" in match.group().lower() or b"permission" in match.group().lower()
    ]
    print("\n".join(diagnostics[-5:]), file=sys.stderr)
    print(f"FAIL: {expected} not rendered", file=sys.stderr)
    raise SystemExit(1)
print(f"PASS: {expected} rendered")
