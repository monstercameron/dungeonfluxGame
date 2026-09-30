#!/usr/bin/env python3
"""Verify the pinned, read-only dependency audit inputs from this checkout."""
from __future__ import annotations

import hashlib
import json
import os
import stat

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), os.pardir))
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


def open_root_directory() -> int:
    required_flags = ("O_NOFOLLOW", "O_DIRECTORY", "O_NONBLOCK", "O_CLOEXEC")
    if os.name != "posix" or os.open not in os.supports_dir_fd:
        raise ValueError("anchored no-follow directory opens are unsupported")
    if any(not hasattr(os, flag) for flag in required_flags):
        raise ValueError("required no-follow open flags are unsupported")

    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    owned_fds: set[int] = set()
    try:
        current_fd = os.open(os.path.sep, directory_flags)
        owned_fds.add(current_fd)
        for component in ROOT.split(os.path.sep):
            if not component:
                continue
            child_fd = os.open(component, directory_flags, dir_fd=current_fd)
            owned_fds.add(child_fd)
            os.close(current_fd)
            owned_fds.remove(current_fd)
            current_fd = child_fd
        if not stat.S_ISDIR(os.fstat(current_fd).st_mode):
            raise ValueError("checkout root is not a directory")
        owned_fds.remove(current_fd)
        return current_fd
    finally:
        for descriptor in reversed(tuple(owned_fds)):
            os.close(descriptor)


def open_input_beneath_root(relative: str) -> int:
    components = relative.split("/")
    if not components or any(part in ("", ".", "..") for part in components):
        raise ValueError(f"invalid pinned input name: {relative}")

    owned_fds: set[int] = set()
    leaf_fd = -1
    try:
        current_fd = open_root_directory()
        owned_fds.add(current_fd)
        directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
        for component in components[:-1]:
            child_fd = os.open(component, directory_flags, dir_fd=current_fd)
            owned_fds.add(child_fd)
            os.close(current_fd)
            owned_fds.remove(current_fd)
            current_fd = child_fd
        leaf_flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
        leaf_fd = os.open(components[-1], leaf_flags, dir_fd=current_fd)
        owned_fds.add(leaf_fd)
        for descriptor in tuple(owned_fds):
            if descriptor != leaf_fd:
                os.close(descriptor)
                owned_fds.remove(descriptor)
        owned_fds.remove(leaf_fd)
        return leaf_fd
    finally:
        for descriptor in reversed(tuple(owned_fds)):
            os.close(descriptor)


def read_regular(relative: str) -> bytes:
    file_fd = -1
    try:
        file_fd = open_input_beneath_root(relative)
        info = os.fstat(file_fd)
        if not stat.S_ISREG(info.st_mode) or info.st_size > MAX_BYTES:
            raise ValueError(f"input is not a bounded regular file: {relative}")
        with os.fdopen(file_fd, "rb", closefd=True) as source:
            file_fd = -1
            payload = source.read(MAX_BYTES + 1)
        if len(payload) > MAX_BYTES:
            raise ValueError(f"input exceeds byte bound: {relative}")
        return payload
    except FileNotFoundError as error:
        raise ValueError(f"missing input: {relative}") from error
    except OSError as error:
        raise ValueError(f"unsafe or unavailable input: {relative}") from error
    finally:
        if file_fd >= 0:
            os.close(file_fd)


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
