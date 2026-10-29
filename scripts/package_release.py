#!/usr/bin/env python3
"""Build desktop release packages using platform tools and the Python standard library."""

import argparse
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def licenses(destination):
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "LICENSE", destination / "LICENSE")
    shutil.copy2(ROOT / "assets/fonts/OFL.txt", destination / "FONT-LICENSE.txt")
    shutil.copy2(ROOT / "assets/fonts/Inter-OFL.txt", destination / "INTER-LICENSE.txt")
    shutil.copy2(ROOT / "assets/fonts/Bungee-OFL.txt", destination / "BUNGEE-LICENSE.txt")


def macos(binary, version, arch, output, stage):
    image = stage / "image"
    contents = image / "JSON Lens.app/Contents"
    executable = contents / "MacOS/json-lens"
    executable.parent.mkdir(parents=True)
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    licenses(contents / "Resources")
    shutil.copy2(ROOT / "packaging/icons/json-lens.icns", contents / "Resources/json-lens.icns")
    with (contents / "Info.plist").open("wb") as file:
        plistlib.dump({
            "CFBundleName": "JSON Lens",
            "CFBundleDisplayName": "JSON Lens",
            "CFBundleIdentifier": "io.github.sihaixianyu.json-lens",
            "CFBundleExecutable": "json-lens",
            "CFBundleIconFile": "json-lens.icns",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version.split("-")[0].split("+")[0],
            "CFBundleVersion": version.split("-")[0].split("+")[0],
            "LSMinimumSystemVersion": "11.0",
            "NSHighResolutionCapable": True,
            "NSHumanReadableCopyright": "Apache-2.0; bundled font licensed under SIL OFL 1.1",
        }, file)
    os.symlink("/Applications", image / "Applications")
    run("codesign", "--force", "--deep", "--sign", "-", str(contents.parent))
    run("codesign", "--verify", "--deep", "--strict", str(contents.parent))
    run("hdiutil", "create", "-volname", "JSON Lens", "-srcfolder", str(image),
        "-fs", "HFS+", "-nospotlight", "-format", "UDZO", "-ov",
        str(output / f"json-lens-macos-{arch}-{version}.dmg"))


def windows(binary, version, arch, output, stage):
    # A standalone binary is useful alongside the installer.
    shutil.copy2(binary, output / f"json-lens-windows-{arch}-{version}.exe")
    shutil.copy2(binary, stage / "json-lens.exe")
    shutil.copy2(ROOT / "packaging/icons/json-lens.ico", stage / "json-lens.ico")
    licenses(stage)
    installer = output / f"json-lens-windows-{arch}-{version}-setup.exe"
    nsis = shutil.which("makensis")
    if not nsis:
        nsis = str(Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "NSIS/makensis.exe")
    run(nsis, f"/DAPP_VERSION={version}", f"/DSTAGE={stage}", f"/DOUTPUT={installer}",
        str(ROOT / "packaging/windows.nsi"))


def linux(binary, version, arch, output, stage):
    portable = stage / "portable/json-lens"
    portable.mkdir(parents=True)
    executable = portable / "json-lens"
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    licenses(portable)
    shutil.copy2(ROOT / "packaging/json-lens.svg", portable / "json-lens.svg")
    shutil.copy2(ROOT / "packaging/icons/json-lens-256.png", portable / "json-lens.png")
    shutil.copy2(ROOT / "packaging/json-lens.desktop", portable / "json-lens.desktop")
    with tarfile.open(output / f"json-lens-linux-{arch}-{version}.tar.gz", "w:gz") as archive:
        archive.add(portable, arcname="json-lens")

    package = stage / "deb"
    (package / "usr/bin").mkdir(parents=True)
    shutil.copy2(executable, package / "usr/bin/json-lens")
    licenses(package / "usr/share/doc/json-lens")
    for resource, destination in [
        ("json-lens.desktop", "usr/share/applications/json-lens.desktop"),
        ("json-lens.svg", "usr/share/icons/hicolor/scalable/apps/json-lens.svg"),
    ]:
        target = package / destination
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "packaging" / resource, target)

    for size in (16, 24, 32, 48, 64, 128, 256, 512):
        target = package / f"usr/share/icons/hicolor/{size}x{size}/apps/json-lens.png"
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / f"packaging/icons/json-lens-{size}.png", target)

    # Derive runtime library requirements from the actual executable.
    debian = stage / "debian"
    debian.mkdir()
    (debian / "control").write_text(
        "Source: json-lens\nSection: utils\nPriority: optional\n"
        "Maintainer: JSON Lens contributors <noreply@github.com>\n\n"
        "Package: json-lens\nArchitecture: any\nDescription: Native JSON editor\n",
        encoding="utf-8",
    )
    requirements = run("dpkg-shlibdeps", "-O", "-e" + str(executable), cwd=stage,
                       capture_output=True, text=True).stdout
    depends = next(line.removeprefix("shlibs:Depends=") for line in requirements.splitlines()
                   if line.startswith("shlibs:Depends="))
    # wgpu/winit load these graphics libraries dynamically, so shlibdeps cannot see them.
    depends += ", libvulkan1, libx11-6, libxcursor1, libxi6, libxrandr2, libxkbcommon0, libwayland-client0"
    control = package / "DEBIAN"
    control.mkdir()
    deb_version = version.replace("-", "~")
    installed_size = sum(file.stat().st_size for file in (package / "usr").rglob("*") if file.is_file())
    (control / "control").write_text(
        f"Package: json-lens\nVersion: {deb_version}\nArchitecture: amd64\n"
        "Section: utils\nPriority: optional\n"
        "Maintainer: JSON Lens contributors <noreply@github.com>\n"
        f"Depends: {depends}\nInstalled-Size: {(installed_size + 1023) // 1024}\n"
        "Homepage: https://github.com/sihaixianyu/json-lens\n"
        "Description: Native JSON editor with graph and comparison views\n"
        " Edit, format, search and compare JSON with GPU rendering.\n",
        encoding="utf-8",
    )
    run("dpkg-deb", "--build", "--root-owner-group", str(package),
        str(output / f"json-lens-linux-{arch}-{version}.deb"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["macos", "windows", "linux"])
    parser.add_argument("--version", required=True)
    parser.add_argument("--arch", required=True, choices=["x86_64", "aarch64"])
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "release-dist")
    parser.add_argument("--stage", type=Path, default=ROOT / "target/package")
    args = parser.parse_args()
    if not re.fullmatch(r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", args.version):
        parser.error("version must be a semantic version without leading v")
    if args.platform != "macos" and args.arch != "x86_64":
        parser.error("Linux and Windows packages currently support x86_64")
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    # Never remove an existing directory supplied by the caller.
    stage = args.stage.resolve() / f"{args.platform}-{args.arch}-{args.version}"
    stage.mkdir(parents=True, exist_ok=False)
    output.mkdir(parents=True, exist_ok=True)
    globals()[args.platform](binary, args.version, args.arch, output, stage)


if __name__ == "__main__":
    main()
