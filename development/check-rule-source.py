#!/usr/bin/env python3
"""Bounded integrity check for the retained public SRD source pin."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import Any

MAX_SOURCE_BYTES = 64 * 1024 * 1024
MAX_MANIFEST_BYTES = 1024 * 1024
PINNED_ATTRIBUTION = (
    'This work includes material from the System Reference Document 5.2.1 '
    '("SRD 5.2.1") by Wizards of the Coast LLC, available at '
    'https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the '
    'Creative Commons Attribution 4.0 International License, available at '
    'https://creativecommons.org/licenses/by/4.0/legalcode.'
)
EXPECTED = {
    "schema_version": 1,
    "source_kind": "public_srd_subset",
    "publisher": "Wizards of the Coast LLC",
    "title": "System Reference Document 5.2.1",
    "version": "5.2.1",
    "language": "English",
    "publisher_page": "https://www.dndbeyond.com/srd",
    "download_url": "https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf",
    "license_name": "Creative Commons Attribution 4.0 International",
    "license_identifier": "CC-BY-4.0",
    "license_url": "https://creativecommons.org/licenses/by/4.0/legalcode",
    "book_titles": (
        "2024 Player's Handbook",
        "2024 Dungeon Master's Guide",
        "2025 Monster Manual",
    ),
}


class VerificationError(Exception):
    """A source pin is invalid or incomplete."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise VerificationError(message)


def file_sha256(path: Path, failure_message: str) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source_stream:
            for chunk in iter(lambda: source_stream.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise VerificationError(failure_message) from error
    return digest.hexdigest()


def normalized(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "", text.lower())


def read_manifest(path: Path) -> dict[str, Any]:
    descriptor = None
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NONBLOCK", 0))
        require(stat.S_ISREG(os.fstat(descriptor).st_mode), "manifest must be a regular file")
        contents = bytearray()
        while len(contents) <= MAX_MANIFEST_BYTES:
            chunk = os.read(descriptor, min(65536, MAX_MANIFEST_BYTES + 1 - len(contents)))
            if not chunk:
                break
            contents.extend(chunk)
        require(len(contents) <= MAX_MANIFEST_BYTES, "manifest exceeds 1 MiB")
        value = json.loads(contents.decode("utf-8"))
    except VerificationError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise VerificationError(f"cannot read valid JSON manifest: {error}") from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
    require(isinstance(value, dict), "manifest root must be an object")
    return value


def verify_full_book_sources(manifest_dir: Path, books: list[Any]) -> None:
    for book in books:
        title = book.get("title")
        require(book.get("status") == "acquired", f"full-source check refused: {title} bytes are unavailable")
        revision = book.get("revision")
        require(isinstance(revision, str) and revision.strip(), f"full-source check refused: {title} revision is unavailable")
        filename = book.get("file")
        require(isinstance(filename, str) and filename == Path(filename).name, f"full-source check refused: {title} file pin is unavailable")
        try:
            source_path = (manifest_dir / filename).resolve()
        except (OSError, RuntimeError) as error:
            raise VerificationError(f"full-source check refused: {title} file path is invalid") from error
        require(source_path.parent == manifest_dir, f"full-source check refused: {title} file path escapes manifest directory")
        try:
            size = source_path.stat().st_size
        except OSError as error:
            raise VerificationError(f"full-source check refused: {title} bytes are unavailable") from error
        require(0 < size <= MAX_SOURCE_BYTES, f"full-source check refused: {title} file is empty or exceeds 64 MiB")
        require(book.get("size_bytes") == size, f"full-source check refused: {title} byte size does not match")
        expected_digest = book.get("sha256")
        require(isinstance(expected_digest, str) and re.fullmatch(r"[0-9a-f]{64}", expected_digest) is not None, f"full-source check refused: {title} SHA-256 pin is missing")
        actual_digest = file_sha256(source_path, f"full-source check refused: {title} bytes are unreadable")
        require(actual_digest == expected_digest, f"full-source check refused: {title} SHA-256 does not match")


def verify(manifest_path: Path, require_full_book_sources: bool = False) -> dict[str, Any]:
    manifest = read_manifest(manifest_path)
    for key, expected in EXPECTED.items():
        if key == "license_name":
            actual = manifest.get("license", {}).get("name") if isinstance(manifest.get("license"), dict) else None
        elif key == "license_identifier":
            actual = manifest.get("license", {}).get("identifier") if isinstance(manifest.get("license"), dict) else None
        elif key == "license_url":
            actual = manifest.get("license", {}).get("url") if isinstance(manifest.get("license"), dict) else None
        elif key == "book_titles":
            books = manifest.get("scope", {}).get("required_full_book_sources") if isinstance(manifest.get("scope"), dict) else None
            actual = tuple(book.get("title") for book in books) if isinstance(books, list) and all(isinstance(book, dict) for book in books) else None
        else:
            actual = manifest.get(key)
        require(actual == expected, f"pinned metadata mismatch for {key}")

    require(manifest.get("acquired_utc_date") == "2026-09-30", "acquisition date is missing or unexpected")
    require(
        manifest.get("acquisition_method")
        == "Unauthenticated HTTPS download from the English SRD v5.2.1 link on the publisher page",
        "acquisition provenance is missing or unexpected",
    )
    scope = manifest.get("scope")
    require(isinstance(scope, dict), "scope must be an object")
    require(scope.get("coverage_claim") == "partial_srd_contents_only", "source must remain explicitly partial")
    require(scope.get("full_2024_rules_coverage") is False, "full 2024 rules coverage cannot be claimed")
    require(scope.get("full_catalog_denominator") == "unresolved", "full catalog denominator must remain unresolved")
    require(scope.get("non_srd_rights_grant_review") == "unresolved", "non-SRD rights review must remain unresolved")
    require(scope.get("production_ruleset_id") is None, "this task cannot assign a production RulesetId")
    books = scope.get("required_full_book_sources")
    require(isinstance(books, list), "required full-book source list is missing")
    for book in books:
        require(isinstance(book, dict), "full-book source entry must be an object")
        status = book.get("status")
        require(status in ("unacquired", "acquired"), f"required source has an invalid acquisition status: {book.get('title')}")
        if status == "unacquired":
            require(book.get("revision") is None, f"unacquired source has a fabricated revision: {book.get('title')}")
        else:
            require(require_full_book_sources, f"acquired source requires --require-full-book-sources: {book.get('title')}")

    filename = manifest.get("file")
    require(isinstance(filename, str) and filename == Path(filename).name, "source file must be a basename")
    try:
        manifest_dir = manifest_path.resolve().parent
        source_path = (manifest_dir / filename).resolve()
    except (OSError, RuntimeError) as error:
        raise VerificationError("source file path is invalid") from error
    require(source_path.parent == manifest_dir, "source path escapes manifest directory")
    try:
        source_size = source_path.stat().st_size
    except OSError as error:
        raise VerificationError(f"cannot stat source PDF: {error}") from error
    require(0 < source_size <= MAX_SOURCE_BYTES, "source PDF is empty or exceeds 64 MiB")
    require(manifest.get("size_bytes") == source_size, "source byte size does not match manifest")
    digest = file_sha256(source_path, "cannot read source PDF")
    require(manifest.get("sha256") == digest, "source SHA-256 does not match manifest")
    if require_full_book_sources:
        verify_full_book_sources(manifest_dir, books)

    try:
        from pypdf import PdfReader

        reader = PdfReader(str(source_path), strict=True)
        require(not reader.is_encrypted, "source PDF must not be encrypted")
        require(len(reader.pages) == manifest.get("page_count"), "PDF page count does not match manifest")
        require(len(reader.pages) == 364, "pinned edition page count is not 364")
        legal_text = reader.pages[0].extract_text() or ""
        legal = normalized(legal_text)
        require("systemreferencedocument521" in legal, "PDF title/version is absent from legal page")
        require("creativecommonsattribution40internationallicense" in legal, "CC-BY-4.0 notice is absent from legal page")
        attribution = manifest["license"].get("attribution")
        require(attribution == PINNED_ATTRIBUTION, "required attribution must exactly match the pinned statement")
        require(normalized(PINNED_ATTRIBUTION) in legal, "pinned attribution does not match PDF legal page")
        contents_text = reader.pages[1].extract_text() or ""
        require("Contents" in contents_text and "Playing the Game" in contents_text, "PDF contents locator does not match page 2")
        locators = manifest.get("verified_locators")
        require(isinstance(locators, list) and len(locators) >= 4, "verified legal and contents locators are missing")
        require(any(item.get("kind") == "legal_information" and item.get("pdf_page") == 1 for item in locators if isinstance(item, dict)), "legal page locator is missing")
        require(any(item.get("kind") == "contents" and item.get("pdf_page") == 2 for item in locators if isinstance(item, dict)), "contents page locator is missing")
        for item in locators:
            require(isinstance(item, dict), "page locator must be an object")
            page_number = item.get("pdf_page")
            require(isinstance(page_number, int) and 1 <= page_number <= len(reader.pages), "page locator is out of bounds")
            label = item.get("label")
            require(isinstance(label, str) and label.strip(), "page locator label is missing")
            if item.get("kind") == "section":
                text = normalized(reader.pages[page_number - 1].extract_text() or "")
                require(normalized(label) in text, f"section locator does not match PDF page {page_number}: {label}")
    except VerificationError:
        raise
    except Exception as error:  # pypdf can raise several parser-specific exceptions.
        raise VerificationError(f"cannot parse source PDF: {error}") from error

    return {"result": "verified", "version": EXPECTED["version"], "sha256": digest, "size_bytes": source_size, "page_count": 364, "scope": "partial_srd_contents_only", "full_book_sources": "unacquired"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=Path(__file__).parent / "rules-sources" / "manifest.json")
    parser.add_argument(
        "--require-full-book-sources",
        action="store_true",
        help="require byte- and revision-pinned PHB, DMG, and Monster Manual sources",
    )
    args = parser.parse_args()
    try:
        print(json.dumps(verify(args.manifest, args.require_full_book_sources), sort_keys=True))
        return 0
    except VerificationError as error:
        print(f"source check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
