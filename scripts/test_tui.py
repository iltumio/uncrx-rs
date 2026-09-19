#!/usr/bin/env python3
"""Drive the actual TUI in a Unix PTY and check terminal restoration."""
import fcntl
import io
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time
import zipfile

binary = str(Path(sys.argv[1]).resolve())

def check_case(payload, expected):
    with tempfile.TemporaryDirectory(prefix="uncrx-tui-") as directory:
        Path(directory, "test.crx").write_bytes(payload)
        master, slave = pty.openpty()
        process = None
        try:
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
            before = termios.tcgetattr(slave)
            process = subprocess.Popen([binary], cwd=directory, stdin=slave, stdout=slave,
                                       stderr=slave, env={**os.environ, "TERM": "xterm-256color"})
            output = bytearray()
            def wait_for(marker):
                deadline = time.monotonic() + 10
                while re.sub(rb"\s+", b"", marker) not in re.sub(rb"\s+", b"", re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", output)):
                    if time.monotonic() > deadline:
                        raise AssertionError(f"Missing {marker!r}: {bytes(output[-1500:])!r}")
                    if select.select([master], [], [], 0.1)[0]:
                        chunk = os.read(master, 65536)
                        if not chunk:
                            raise AssertionError("PTY closed early")
                        output.extend(chunk)
            wait_for(b"File Browser")
            # Parent directory is selected initially, then the only CRX file.
            os.write(master, b"j\r")
            wait_for(expected)
            os.write(master, b"q")
            assert process.wait(timeout=10) == 0
            assert termios.tcgetattr(slave) == before, "Terminal mode was not restored"
            if expected == b"Extraction successful":
                assert Path(directory, "out/test/file.txt").read_bytes() == b"hello"
            else:
                assert not Path(directory, "out/test").exists()
        finally:
            if process is not None and process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)
            os.close(slave)

archive = io.BytesIO()
with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as writer:
    writer.writestr("file.txt", b"hello")
check_case(b"Cr24" + struct.pack("<III", 2, 0, 0) + archive.getvalue(), b"Extraction successful")
check_case(b"Cr24" + struct.pack("<II", 2, 0xffffffff), b"Error occurred")
print("TUI PTY: extraction, malformed input, and terminal restoration passed")
