"""Collect required release files and generate a complete updater manifest."""
from __future__ import annotations

import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shutil


ARTIFACTS = {
    "windows": [("nsis/*.exe", "windows-setup.exe"), ("nsis/*.exe.sig", "windows-setup.exe.sig")],
    "macos": [("dmg/*.dmg", "macos.dmg"), ("macos/*.app.tar.gz", "macos.app.tar.gz"),
              ("macos/*.app.tar.gz.sig", "macos.app.tar.gz.sig")],
    "linux": [("deb/*.deb", "linux.deb"), ("appimage/*.AppImage", "linux.AppImage"),
              ("appimage/*.AppImage.sig", "linux.AppImage.sig")],
}
UPDATERS = {"windows-x86_64": "windows-setup.exe", "darwin-aarch64": "macos.app.tar.gz",
            "linux-x86_64": "linux.AppImage"}


def artifact_name(version: str, suffix: str) -> str:
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Version must be x.y.z")
    return f"pg-model-viewer-{version}-{suffix}"


def require_file(path: Path) -> None:
    if not path.is_file() or path.stat().st_size == 0:
        raise ValueError(f"Required release file is missing or empty: {path}")


def read_signature(path: Path) -> str:
    require_file(path)
    signature = path.read_text(encoding="utf-8").strip()
    try:
        if not base64.b64decode(signature, validate=True):
            raise ValueError("empty signature")
    except ValueError as error:
        raise ValueError(f"Invalid updater signature: {path}") from error
    return signature


def collect(bundle_dir: Path, destination: Path, platform: str, version: str) -> None:
    files = []
    for pattern, suffix in ARTIFACTS[platform]:
        matches = sorted(bundle_dir.glob(pattern))
        if len(matches) != 1:
            raise ValueError(f"Expected exactly one {pattern} in {bundle_dir}; found {len(matches)}")
        require_file(matches[0])
        if suffix.endswith(".sig"):
            read_signature(matches[0])
        files.append((matches[0], destination / artifact_name(version, suffix)))
    destination.mkdir(parents=True, exist_ok=True)
    for source, target in files:
        shutil.copy2(source, target)
        print(f"Collected {target.name}")


def manifest(directory: Path, repository: str, version: str, public_key: str) -> dict:
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("Repository must be owner/name")
    # Check the download installers too, not only the three updater payloads.
    for artifacts in ARTIFACTS.values():
        for _, suffix in artifacts:
            require_file(directory / artifact_name(version, suffix))
    platforms = {}
    for platform, suffix in UPDATERS.items():
        filename = artifact_name(version, suffix)
        artifact = directory / filename
        signature_path = directory / (filename + ".sig")
        signature = read_signature(signature_path)
        verify_signature(public_key, artifact, signature_path)
        platforms[platform] = {
            "url": f"https://github.com/{repository}/releases/download/v{version}/{filename}",
            "signature": signature,
        }
    result = {"version": version, "notes": "See the release page for details.",
              "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
              "platforms": platforms}
    (directory / "latest.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return result


def verify_signature(public_key: str, artifact: Path, signature_path: Path) -> None:
    # PyNaCl is a CI-only dependency; it is not included in the frozen extractor.
    from nacl.exceptions import BadSignatureError
    from nacl.signing import VerifyKey
    try:
        public_lines = base64.b64decode(public_key, validate=True).decode().splitlines()
        signature_lines = base64.b64decode(read_signature(signature_path), validate=True).decode().splitlines()
        public = base64.b64decode(public_lines[1], validate=True)
        signature = base64.b64decode(signature_lines[1], validate=True)
        if len(public) != 42 or public[:2] != b"Ed" or len(signature) != 74 or public[2:10] != signature[2:10]:
            raise ValueError("Key or signature ID mismatch")
        if signature[:2] == b"ED":
            digest = hashlib.blake2b(digest_size=64)
            with artifact.open("rb") as stream:
                for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(chunk)
            message = digest.digest()
        elif signature[:2] == b"Ed":
            message = artifact.read_bytes()
        else:
            raise ValueError("Unsupported signature algorithm")
        verifier = VerifyKey(public[10:])
        verifier.verify(message, signature[10:])
        trusted_prefix = "trusted comment: "
        if not signature_lines[2].startswith(trusted_prefix):
            raise ValueError("Missing trusted comment")
        verifier.verify(signature[10:] + signature_lines[2][len(trusted_prefix):].encode(),
                        base64.b64decode(signature_lines[3], validate=True))
    except (ValueError, IndexError, BadSignatureError) as error:
        raise ValueError(f"Signature verification failed for {artifact.name}") from error


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    collect_parser = sub.add_parser("collect")
    collect_parser.add_argument("--bundle-dir", type=Path, required=True)
    collect_parser.add_argument("--destination", type=Path, required=True)
    collect_parser.add_argument("--platform", choices=ARTIFACTS, required=True)
    collect_parser.add_argument("--version", required=True)
    manifest_parser = sub.add_parser("manifest")
    manifest_parser.add_argument("--directory", type=Path, required=True)
    manifest_parser.add_argument("--repository", required=True)
    manifest_parser.add_argument("--version", required=True)
    manifest_parser.add_argument("--config", type=Path, default=Path("src-tauri/tauri.conf.json"))
    verify_parser = sub.add_parser("verify")
    verify_parser.add_argument("artifact", type=Path)
    verify_parser.add_argument("--config", type=Path, default=Path("src-tauri/tauri.conf.json"))
    args = parser.parse_args()
    try:
        if args.command == "collect":
            collect(args.bundle_dir, args.destination, args.platform, args.version)
        else:
            key = json.loads(args.config.read_text(encoding="utf-8"))["plugins"]["updater"]["pubkey"]
            if args.command == "manifest":
                manifest(args.directory, args.repository, args.version, key)
            else:
                verify_signature(key, args.artifact, Path(str(args.artifact) + ".sig"))
    except (ValueError, OSError) as error:
        parser.exit(1, f"ERROR: {error}\n")


if __name__ == "__main__":
    main()
