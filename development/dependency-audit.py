#!/usr/bin/env python3
"""Verify the pinned, read-only dependency audit inputs from this checkout."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import stat
import sys

ROOT = Path(__file__).resolve().parent.parent
AUDIT = ROOT / "development" / "dependency-audit"
MAX_BYTES = 32 * 1024 * 1024
EXPECTED_REPORT_SHA256 = "d569532d942bce4d22b7107f04b969e0a16addc9bc19fe4d59afd523d3fd478c"
PINNED_INPUTS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "crates/df-protocol/Cargo.toml",
    "crates/df-rpc-bridge/Cargo.toml",
    "crates/df-observe/Cargo.toml",
    "crates/df-tools/Cargo.toml",
    "crates/df-rpc-bridge/vendor/h2/Cargo.toml",
    "development/dependency-audit/metadata/native.json",
    "development/dependency-audit/metadata/wasm.json",
    "development/dependency-audit/packages.json",
)


def read_regular(relative: str) -> bytes:
    path = ROOT / relative
    try:
        info = path.lstat()
    except OSError as error:
        raise ValueError(f"missing input: {relative}") from error
    if not stat.S_ISREG(info.st_mode) or info.st_size > MAX_BYTES:
        raise ValueError(f"input is not a bounded regular file: {relative}")
    try:
        with path.open("rb") as source:
            payload = source.read(MAX_BYTES + 1)
    except OSError as error:
        raise ValueError(f"unreadable input: {relative}") from error
    if len(payload) > MAX_BYTES:
        raise ValueError(f"input exceeds byte bound: {relative}")
    return payload


def sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def verify() -> dict[str, object]:
    report_bytes = read_regular("development/dependency-audit/report.json")
    if sha256(report_bytes) != EXPECTED_REPORT_SHA256:
        raise ValueError("audit report differs from its pinned bytes")
    report = json.loads(report_bytes)
    if not isinstance(report, dict) or report.get("schema") != 1:
        raise ValueError("malformed audit report")
    expected_inputs = report.get("input_sha256")
    if not isinstance(expected_inputs, dict) or set(expected_inputs) != set(PINNED_INPUTS):
        raise ValueError("audit report input identity set is incomplete or unexpected")
    for relative in PINNED_INPUTS:
        if sha256(read_regular(relative)) != expected_inputs[relative]:
            raise ValueError(f"stale or changed pinned input: {relative}")

    package_data = json.loads(read_regular("development/dependency-audit/packages.json"))
    if not isinstance(package_data, dict) or not isinstance(package_data.get("packages"), list):
        raise ValueError("malformed package evidence")
    package_ids = [item.get("id") for item in package_data["packages"] if isinstance(item, dict)]
    if len(package_ids) != len(package_data["packages"]) or len(set(package_ids)) != len(package_ids):
        raise ValueError("malformed or duplicate package identity")
    if sorted(package_ids) != report.get("package_ids"):
        raise ValueError("package identity set is omitted, added, or reordered")

    target_identity_union: set[str] = set()
    for target, relative in (
        ("aarch64-apple-darwin", "development/dependency-audit/metadata/native.json"),
        ("wasm32-unknown-unknown", "development/dependency-audit/metadata/wasm.json"),
    ):
        metadata = json.loads(read_regular(relative))
        if (metadata.get("target_directory") is None or metadata.get("resolve") is None
                or metadata.get("workspace_root") != report.get("captured_workspace_root")):
            raise ValueError(f"malformed Cargo metadata for {target}")
        metadata_ids = sorted(package["id"] for package in metadata.get("packages", []))
        target_identity_union.update(metadata_ids)
        declared_ids = report.get("target_package_ids", {}).get(target)
        if metadata_ids != declared_ids:
            raise ValueError(f"Cargo package identities differ for {target}")
    if sorted(target_identity_union) != package_ids:
        raise ValueError("package identity set differs from the union of target metadata")
    return {"ok": True, "packages": len(package_ids), "targets": 2}


def main() -> int:
    try:
        result = verify()
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(json.dumps({"ok": False, "reason": str(error)}, sort_keys=True))
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
