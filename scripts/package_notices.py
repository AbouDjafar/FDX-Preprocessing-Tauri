"""Collect installed license texts for redistribution without modifying dependencies."""

import importlib.metadata
import shutil
import sys
from pathlib import Path


def collect(destination):
    root = Path(destination)
    root.mkdir(parents=True, exist_ok=True)
    python_license = Path(sys.base_prefix) / "LICENSE.txt"
    if python_license.is_file():
        shutil.copy2(python_license, root / "CPython-LICENSE.txt")
    supplement = Path(__file__).resolve().parents[1] / "packaging" / "third_party_licenses"
    if supplement.is_dir():
        shutil.copytree(supplement, root / "supplement", dirs_exist_ok=True)
    icons_license = Path(__file__).resolve().parents[1] / "src/fdx_atelier/assets/icons/LICENSE"
    if icons_license.exists():
        shutil.copy2(icons_license, root / "Lucide-LICENSE.txt")
    lines = [
        "# Third-party components",
        "",
        "Runtime and build dependencies of FDX Atelier Données.",
        "License texts supplied by their distributions are in this folder.",
        "",
    ]
    for distribution in sorted(importlib.metadata.distributions(), key=lambda d: d.metadata["Name"].lower()):
        name = distribution.metadata["Name"]
        if name.lower() in {
            "fdx-atelier",
            "pip",
            "setuptools",
            "pytest",
            "pytest-qt",
            "hypothesis",
            "ruff",
            "nuitka",
            "py-spy",
            "pefile",
        }:
            continue
        license_name = distribution.metadata.get("License-Expression") or distribution.metadata.get(
            "License", "See bundled license"
        )
        lines.append(f"- {name} {distribution.version}: {license_name.splitlines()[0][:180]}")
        for file in distribution.files or []:
            normalized = str(file).replace("\\", "/")
            if ".dist-info/licenses/" in normalized or Path(file).name.lower().startswith(
                ("license", "licence", "copying", "notice")
            ):
                source = Path(distribution.locate_file(file))
                if source.is_file():
                    relative = (
                        normalized.split("licenses/", 1)[1]
                        if ".dist-info/licenses/" in normalized
                        else Path(file).name
                    )
                    target = root / name / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source, target)
    lines.extend(
        [
            "",
            "Qt for Python: https://www.qt.io/qt-for-python",
            "Qt licensing and source access: https://doc.qt.io/qtforpython-6/licenses.html",
        ]
    )
    (root / "THIRD_PARTY_NOTICES.md").write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    import sys

    collect(sys.argv[1])
