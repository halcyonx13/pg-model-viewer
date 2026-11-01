"""Release packaging/verification tests; needs tools/release-requirements.txt."""
import base64
import contextlib
import hashlib
import io
from pathlib import Path
import tempfile
import unittest

from nacl.signing import SigningKey

from release_artifacts import ARTIFACTS, artifact_name, collect, manifest, verify_signature


def encoded(text):
    return base64.b64encode(text).decode()


class ReleaseArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = self.enterContext(tempfile.TemporaryDirectory())
        self.directory = Path(self.temp)
        self.enterContext(contextlib.redirect_stdout(io.StringIO()))
        self.key = SigningKey.generate()
        self.key_id = b"test-key"
        public = b"Ed" + self.key_id + bytes(self.key.verify_key)
        self.public = encoded(("untrusted comment: test key\n" + encoded(public) + "\n").encode())
        self.version = "0.2.0"

    def signature(self, content):
        signature = self.key.sign(hashlib.blake2b(content, digest_size=64).digest()).signature
        record = b"ED" + self.key_id + signature
        trusted = "timestamp:1\tfile:test\tprehashed"
        global_signature = self.key.sign(signature + trusted.encode()).signature
        return encoded(("untrusted comment: test signature\n" + encoded(record)
                        + "\ntrusted comment: " + trusted + "\n" + encoded(global_signature) + "\n").encode())

    def make_artifacts(self):
        content = b"release artifact test payload\n"
        for artifacts in ARTIFACTS.values():
            for _, suffix in artifacts:
                path = self.directory / artifact_name(self.version, suffix)
                path.write_bytes(self.signature(content).encode() if suffix.endswith(".sig") else content)

    def test_collect_requires_all_platform_files(self):
        bundle = self.directory / "bundle"
        (bundle / "nsis").mkdir(parents=True)
        (bundle / "nsis/app.exe").write_bytes(b"installer")
        destination = self.directory / "out"
        with self.assertRaises(ValueError):
            collect(bundle, destination, "windows", self.version)
        self.assertFalse(destination.exists())
        (bundle / "nsis/app.exe.sig").write_text(self.signature(b"installer"))
        collect(bundle, destination, "windows", self.version)
        self.assertEqual(len(list(destination.iterdir())), 2)

    def test_collect_rejects_ambiguous_installer(self):
        bundle = self.directory / "bundle"
        (bundle / "nsis").mkdir(parents=True)
        for name in ["a.exe", "b.exe"]:
            (bundle / "nsis" / name).write_bytes(b"installer")
        with self.assertRaises(ValueError):
            collect(bundle, self.directory / "out", "windows", self.version)

    def test_manifest_contains_all_verified_platforms(self):
        self.make_artifacts()
        result = manifest(self.directory, "halcyonx13/pg-model-viewer", self.version, self.public)
        self.assertEqual(set(result["platforms"]), {"windows-x86_64", "darwin-aarch64", "linux-x86_64"})
        self.assertIn("halcyonx13/pg-model-viewer/releases/download/v0.2.0/", result["platforms"]["windows-x86_64"]["url"])
        self.assertTrue((self.directory / "latest.json").is_file())

    def test_missing_signature_prevents_manifest(self):
        self.make_artifacts()
        (self.directory / artifact_name(self.version, "linux.AppImage.sig")).unlink()
        with self.assertRaises(ValueError):
            manifest(self.directory, "halcyonx13/pg-model-viewer", self.version, self.public)
        self.assertFalse((self.directory / "latest.json").exists())

    def test_empty_installer_prevents_manifest(self):
        self.make_artifacts()
        (self.directory / artifact_name(self.version, "macos.dmg")).write_bytes(b"")
        with self.assertRaises(ValueError):
            manifest(self.directory, "halcyonx13/pg-model-viewer", self.version, self.public)

    def test_tampered_artifact_prevents_manifest(self):
        self.make_artifacts()
        (self.directory / artifact_name(self.version, "windows-setup.exe")).write_bytes(b"modified")
        with self.assertRaises(ValueError):
            manifest(self.directory, "halcyonx13/pg-model-viewer", self.version, self.public)
        self.assertFalse((self.directory / "latest.json").exists())

    def test_tampered_trusted_comment_is_rejected(self):
        artifact = self.directory / "payload"
        artifact.write_bytes(b"payload")
        signature = self.directory / "payload.sig"
        text = base64.b64decode(self.signature(b"payload")).replace(b"timestamp:1", b"timestamp:2")
        signature.write_text(encoded(text))
        with self.assertRaises(ValueError):
            verify_signature(self.public, artifact, signature)

    def test_version_cannot_escape_artifact_directory(self):
        with self.assertRaises(ValueError):
            artifact_name("../../outside", "windows-setup.exe")


if __name__ == "__main__":
    unittest.main()
