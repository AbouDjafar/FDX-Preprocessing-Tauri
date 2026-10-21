from pathlib import Path

from PySide6.QtCore import QRect, Qt
from PySide6.QtGui import QColor, QFont, QImage, QPainter
from PySide6.QtWidgets import QApplication

from fdx_atelier.ui.fonts import ensure_fonts

app = QApplication.instance() or QApplication([])
ensure_fonts()
root = Path(__file__).resolve().parents[1]
(root / "assets").mkdir(exist_ok=True)
image = QImage(256, 256, QImage.Format.Format_ARGB32)
image.fill(QColor("#183c46"))
painter = QPainter(image)
painter.setRenderHint(QPainter.RenderHint.Antialiasing)
painter.fillRect(QRect(28, 188, 200, 12), QColor("#45bc9b"))
painter.setPen(QColor("#ffffff"))
painter.setFont(QFont("Segoe UI", 64, QFont.Weight.Bold))
painter.drawText(image.rect().adjusted(0, -12, 0, -12), Qt.AlignmentFlag.AlignCenter, "FDX")
painter.end()
if not image.save(str(root / "assets" / "fdx.ico")):
    raise RuntimeError("Ecriture de l'icone impossible")
image.save(str(root / "assets" / "fdx.png"))
