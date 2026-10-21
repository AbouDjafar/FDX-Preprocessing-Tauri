import os
import sys
from pathlib import Path

from PyInstaller.utils.hooks import collect_data_files, collect_submodules, copy_metadata

# Resolve Windows libraries from Windows, never unrelated applications in PATH.
# In particular, third-party icuuc.dll builds do not expose the Windows ICU API.
windows = Path(os.environ["WINDIR"])
os.environ["PATH"] = os.pathsep.join(str(p) for p in (
    windows / "System32", windows, Path(sys.base_prefix), Path(sys.base_prefix) / "DLLs",
))

root = Path(SPECPATH).parent
hidden = collect_submodules("phonenumbers.data") + collect_submodules("pandera.backends.pandas")
hidden += collect_submodules("pandera.engines")
metadata = []
for name in ("pandera", "pandas", "numpy", "pydantic", "typeguard", "typing_extensions"):
    metadata += copy_metadata(name)

analysis = Analysis(
    [str(root / "main.py")],
    pathex=[str(root / "src")],
    binaries=[],
    datas=metadata + collect_data_files("fdx_atelier", includes=["assets/icons/*"]),
    hiddenimports=hidden,
    hookspath=[],
    runtime_hooks=[],
    excludes=["pytest", "hypothesis", "IPython", "matplotlib", "scipy", "PyQt6",
              "phonenumbers.geodata", "phonenumbers.carrierdata", "phonenumbers.tzdata"],
    noarchive=False,
)
archive = PYZ(analysis.pure)
executable = EXE(
    archive, analysis.scripts, [], exclude_binaries=True, name="FDXAtelier",
    debug=False, bootloader_ignore_signals=False, strip=False, upx=False,
    console=False, disable_windowed_traceback=False, icon=str(root / "assets" / "fdx.ico"),
)
distribution = COLLECT(
    executable, analysis.binaries, analysis.datas, strip=False, upx=False, name="FDXAtelier.dist",
)
