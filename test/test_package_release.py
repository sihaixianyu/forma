"""Packaging regressions that run without platform-specific packaging tools."""

from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

from scripts import package_release


class PackageReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "json-lens"
        self.binary.write_bytes(b"test executable")

    def package(self, platform, stage, output):
        arch = "aarch64" if platform == "macos" else "x86_64"
        argv = [
            "package_release.py", platform, "--version", "0.1.1", "--arch", arch,
            "--binary", str(self.binary), "--stage", str(stage), "--output", str(output),
        ]
        with patch("sys.argv", argv):
            package_release.main()

    def test_repeated_runs_preserve_existing_stages_and_use_fresh_directories(self):
        for platform in ("linux", "windows", "macos"):
            with self.subTest(platform=platform):
                arch = "aarch64" if platform == "macos" else "x86_64"
                stage_root = self.root / platform / "stages"
                old_stage = stage_root / f"{platform}-{arch}-0.1.1"
                old_stage.mkdir(parents=True)
                sentinel = old_stage / "keep.txt"
                sentinel.write_text("existing content", encoding="utf-8")
                output = self.root / platform / "output"
                stages = []

                def build(binary, version, arch, output, stage):
                    self.assertEqual(list(stage.iterdir()), [])
                    stages.append(stage)
                    (stage / "nested").mkdir()
                    (output / "artifact").write_bytes(binary.read_bytes())

                with patch.object(package_release, platform, side_effect=build):
                    self.package(platform, stage_root, output)
                    self.package(platform, stage_root, output)

                self.assertEqual(len(set(stages)), 2)
                self.assertTrue(all(not stage.exists() for stage in stages))
                self.assertEqual(sentinel.read_text(encoding="utf-8"), "existing content")
                self.assertEqual(list(stage_root.iterdir()), [old_stage])
                self.assertEqual((output / "artifact").read_bytes(), self.binary.read_bytes())

    def test_failed_packaging_cleans_only_its_temporary_stage(self):
        stage_root = self.root / "stages"
        old_stage = stage_root / "linux-x86_64-0.1.1"
        old_stage.mkdir(parents=True)
        sentinel = old_stage / "keep.txt"
        sentinel.write_text("keep", encoding="utf-8")

        def fail(binary, version, arch, output, stage):
            (stage / "partial").write_text("partial", encoding="utf-8")
            raise RuntimeError("packaging failed")

        with patch.object(package_release, "linux", side_effect=fail):
            with self.assertRaisesRegex(RuntimeError, "packaging failed"):
                self.package("linux", stage_root, self.root / "output")
        self.assertEqual(list(stage_root.iterdir()), [old_stage])
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "keep")

    def test_windows_installer_files_match_staged_payload(self):
        def platform_tool(*args, **kwargs):
            stage = Path(next(arg.removeprefix("/DSTAGE=") for arg in args
                              if arg.startswith("/DSTAGE=")))
            script = Path(args[-1]).read_text(encoding="utf-8")
            installed = set(re.findall(r'^\s*File "\$\{STAGE\}\\([^"\n]+)"', script, re.MULTILINE))
            deleted = set(re.findall(r'^\s*Delete "\$INSTDIR\\([^"\n]+)"', script, re.MULTILINE))
            payload = {file.name for file in stage.iterdir() if file.name != "json-lens.ico"}
            self.assertEqual(installed, payload)
            self.assertTrue(installed <= deleted)
            self.assertIn("PIXELIFY-LICENSE.txt", installed)
            return subprocess.CompletedProcess(args, 0)

        with patch.object(package_release, "run", side_effect=platform_tool) as run:
            self.package("windows", self.root / "stages", self.root / "output")
        run.assert_called_once()

    def test_linux_archive_can_be_built_twice_with_the_same_version(self):
        stage_root = self.root / "stages"
        output = self.root / "output"

        def platform_tool(*args, **kwargs):
            if args[0] == "dpkg-shlibdeps":
                return subprocess.CompletedProcess(args, 0, stdout="shlibs:Depends=libc6\n")
            self.assertEqual(args[0], "dpkg-deb")
            control = (Path(args[-2]) / "DEBIAN/control").read_text(encoding="utf-8")
            self.assertIn("Version: 0.1.1\n", control)
            self.assertIn("Depends: libc6, libvulkan1", control)
            Path(args[-1]).write_bytes(b"test deb package")
            return subprocess.CompletedProcess(args, 0)

        with patch.object(package_release, "run", side_effect=platform_tool):
            self.package("linux", stage_root, output)
            self.package("linux", stage_root, output)

        with tarfile.open(output / "json-lens-linux-x86_64-0.1.1.tar.gz") as archive:
            self.assertEqual(archive.extractfile("json-lens/json-lens").read(), self.binary.read_bytes())
            for name in ("LICENSE", "FONT-LICENSE.txt", "PIXELIFY-LICENSE.txt", "json-lens.png"):
                self.assertIn(f"json-lens/{name}", archive.getnames())
        self.assertTrue((output / "json-lens-linux-x86_64-0.1.1.deb").exists())
        self.assertEqual(list(stage_root.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
