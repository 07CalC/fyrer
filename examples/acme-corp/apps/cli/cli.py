#!/usr/bin/env python3
"""acme-cli: a small multi-purpose demo CLI written in Python."""

import argparse
import ast
import http.server
import json
import os
import pathlib
import time

DIST = pathlib.Path(__file__).resolve().parent / "dist"
SCOPE = os.environ.get("SCOPE", "production")


def cmd_bundle() -> int:
    DIST.mkdir(parents=True, exist_ok=True)
    manifest = {
        "name": "acme-cli",
        "scope": SCOPE,
        "packages": ["ui", "shared", "core"],
        "apps": ["web", "api", "worker", "cli"],
    }
    (DIST / "manifest.json").write_text(json.dumps(manifest, indent=2))
    (DIST / "digest.txt").write_text(
        f"bundle written at {int(time.time())} (scope={SCOPE})\n"
    )
    print(f"[cli] bundle -> {DIST}")
    return 0


def cmd_lint() -> int:
    source_file = pathlib.Path(__file__).resolve()
    ast.parse(source_file.read_text())
    DIST.mkdir(parents=True, exist_ok=True)
    report = DIST / "lint-report.txt"
    report.write_text(f"{source_file.name}: syntax OK\n")
    print(f"[cli] lint OK -> {report}")
    return 0


def cmd_serve() -> int:
    port = int(os.environ.get("PORT", "8000"))
    os.chdir(DIST)
    httpd = http.server.ThreadingHTTPServer(
        ("127.0.0.1", port), http.server.SimpleHTTPRequestHandler
    )
    print(f"[cli] serving {DIST} on http://localhost:{port} (SCOPE={SCOPE}, PORT from task env)")
    httpd.serve_forever()
    return 0


def cmd_slow() -> int:
    print("[cli] sleeping forever (killed by the `timeout` option)...")
    while True:
        time.sleep(60)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(prog="acme-cli")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("bundle", help="build the manifest bundle into dist/")
    sub.add_parser("lint", help="syntax-check cli.py and write a report")
    sub.add_parser("serve", help="serve dist/ over HTTP")
    sub.add_parser("slow", help="sleep forever, used to demo `timeout`")
    args = parser.parse_args()

    dispatch = {
        "bundle": cmd_bundle,
        "lint": cmd_lint,
        "serve": cmd_serve,
        "slow": cmd_slow,
    }
    return dispatch[args.command]()


if __name__ == "__main__":
    raise SystemExit(main())