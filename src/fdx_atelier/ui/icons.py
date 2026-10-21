from functools import lru_cache
from pathlib import Path

from PySide6.QtCore import QByteArray, Qt
from PySide6.QtGui import QIcon, QPainter, QPixmap
from PySide6.QtSvg import QSvgRenderer


@lru_cache(maxsize=100)
def icon(name: str, color="#d4dce6"):
    path = Path(__file__).resolve().parents[1] / "assets" / "icons" / f"{name}.svg"
    if not path.exists():
        return QIcon()
    data = path.read_text(encoding="utf-8").replace("currentColor", color)
    renderer = QSvgRenderer(QByteArray(data.encode()))
    pixmap = QPixmap(64, 64)
    pixmap.fill(Qt.GlobalColor.transparent)
    painter = QPainter(pixmap)
    renderer.render(painter)
    painter.end()
    return QIcon(pixmap)
