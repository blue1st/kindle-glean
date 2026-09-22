#!/usr/bin/env python3
"""
Kindle Scribe Notebook Converter Sidecar Builder
Uses PyInstaller to package convert_notebook.py into a standalone executable.
Output is placed in src-tauri/binaries/
"""

import os
import platform
import subprocess
import sys
from pathlib import Path

def main():
    root_dir = Path(__file__).resolve().parent.parent
    script_path = root_dir / "src-tauri" / "scripts" / "kfx_parser" / "convert_notebook.py"
    output_dir = root_dir / "src-tauri" / "binaries"
    spec_dir = root_dir / "src-tauri" / "target" / "pyinstaller"

    output_dir.mkdir(parents=True, exist_ok=True)
    spec_dir.mkdir(parents=True, exist_ok=True)

    binary_name = "kfx-converter"
    is_windows = platform.system() == "Windows"

    cmd = [
        sys.executable,
        "-m",
        "PyInstaller",
        "--onefile",
        "--clean",
        "--name",
        binary_name,
        "--distpath",
        str(output_dir),
        "--specpath",
        str(spec_dir),
        "--workpath",
        str(spec_dir / "build"),
        # Optimization: exclude unnecessary heavyweight modules
        "--exclude-module", "tkinter",
        "--exclude-module", "matplotlib",
        "--exclude-module", "scipy",
        "--exclude-module", "numpy",
        "--exclude-module", "PIL",
        "--exclude-module", "curses",
        "--exclude-module", "unittest",
        # Source script
        str(script_path),
    ]

    if not is_windows:
        cmd.append("--strip")

    print(f"Building sidecar binary from {script_path}...")
    print(f"Executing: {' '.join(cmd)}")

    res = subprocess.run(cmd, cwd=root_dir)
    if res.returncode != 0:
        print("Error: PyInstaller build failed.", file=sys.stderr)
        sys.exit(res.returncode)

    exe_file = output_dir / (f"{binary_name}.exe" if is_windows else binary_name)
    if exe_file.exists():
        size_mb = exe_file.stat().st_size / (1024 * 1024)
        print(f"Successfully generated sidecar binary: {exe_file} ({size_mb:.2f} MB)")
    else:
        print(f"Warning: Expected output binary {exe_file} was not found.", file=sys.stderr)

if __name__ == "__main__":
    main()
