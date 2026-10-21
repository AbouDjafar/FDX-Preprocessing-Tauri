"""Register installed Windows fonts when Qt's headless platform has no font database."""

from __future__ import annotations

import os
from pathlib import Path

from PySide6.QtGui import QFontDatabase


def ensure_fonts():
    if QFontDatabase.families():
        return
    directory = Path(os.environ.get("WINDIR", "C:/Windows")) / "Fonts"
    for name in ("segoeui.ttf", "segoeuib.ttf", "segoeuii.ttf"):
        path = directory / name
        if path.is_file():
            QFontDatabase.addApplicationFont(str(path))
