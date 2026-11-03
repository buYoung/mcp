"""Byte-preserving stdio <-> one already-warm Unix socket; never starts a product."""
from __future__ import annotations

import os
import socket
import sys
import threading


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: python -m benchmark.stdio_shim SOCKET")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.connect(sys.argv[1])  # No retry/reconnect: an attachment is single use.
        def upload():
            try:
                while data := os.read(sys.stdin.fileno(), 65536):
                    sock.sendall(data)
                sock.shutdown(socket.SHUT_WR)
            except OSError:
                pass
        threading.Thread(target=upload, daemon=True).start()
        while data := sock.recv(65536):
            offset = 0
            while offset < len(data):
                offset += os.write(sys.stdout.fileno(), data[offset:])


if __name__ == "__main__":
    main()
