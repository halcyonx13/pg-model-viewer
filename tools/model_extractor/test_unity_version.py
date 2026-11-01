"""Run with python -m unittest discover -s tools/model_extractor -p 'test_*.py'."""
import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import extract


class UnityVersionTests(unittest.TestCase):
    def setUp(self):
        self.original = extract.UnityPy.config.FALLBACK_UNITY_VERSION
        self.addCleanup(setattr, extract.UnityPy.config, "FALLBACK_UNITY_VERSION", self.original)
        self.output = io.StringIO()
        self.enterContext(contextlib.redirect_stdout(self.output))

    def test_finds_player_above_bundle_directory(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            player = root / "WindowsPlayer.exe"
            player.touch()
            bundles = root / "WindowsPlayer_Data/StreamingAssets/aa/StandaloneWindows64"
            bundles.mkdir(parents=True)
            with patch.object(extract, "windows_product_version",
                              side_effect=lambda path: "6000.4.2f1" if path == player else None):
                extract.configure_unity_version(str(bundles))
            self.assertEqual(extract.UnityPy.config.FALLBACK_UNITY_VERSION, "6000.4.2f1")
            self.assertIn(str(player), self.output.getvalue())

    def test_missing_metadata_uses_logged_default(self):
        with patch.object(extract, "windows_product_version", return_value=None):
            extract.configure_unity_version(".")
        self.assertEqual(extract.UnityPy.config.FALLBACK_UNITY_VERSION, extract.DEFAULT_UNITY_VERSION)
        self.assertIn("player version unavailable", self.output.getvalue())

    def test_override_takes_priority_over_detection(self):
        with patch.object(extract, "windows_product_version") as detect:
            extract.configure_unity_version(".", "6000.5.1f2")
            detect.assert_not_called()
        self.assertEqual(extract.UnityPy.config.FALLBACK_UNITY_VERSION, "6000.5.1f2")

    def test_non_windows_skips_windows_api(self):
        with patch.object(extract.sys, "platform", "linux"):
            self.assertIsNone(extract.windows_product_version(Path(__file__)))


if __name__ == "__main__":
    unittest.main()
