from pathlib import Path

from PySide6.QtGui import QColor, QPalette

STYLE = """
QWidget { color: #e9eaec; font-family: 'Segoe UI'; font-size: 10pt; }
QMainWindow, QStackedWidget, QScrollArea { background: #191a1d; }
QWidget#shell, QWidget#page { background: #191a1d; }
QWidget#topbar { background: #212226; border-bottom: 1px solid #36383d; }
QWidget#navigation { background: #202125; border-right: 1px solid #36383d; }
QLabel#brand { font-size: 15pt; font-weight: 700; }
QLabel#pageTitle { font-size: 20pt; font-weight: 600; }
QLabel#stepNumber, QLabel#eyebrow { color: #a6acb4; font-size: 9pt; }
QLabel[muted=true] { color: #aeb4bd; }
QLabel[heading=true] { font-size: 12pt; font-weight: 600; }
QLabel#metric { font-size: 18pt; font-weight: 600; color: #75dec8; }
QLabel#notice { background: #282b30; border-left: 3px solid #e4bb64; padding: 9px; }
QLabel#success { color: #82e1bd; }
QPushButton, QToolButton { background: #2b2d32; border: 1px solid #43464e; border-radius: 6px; padding: 7px 12px; min-height: 24px; }
QPushButton:hover, QToolButton:hover { background: #373b42; border-color: #838b98; }
QPushButton:pressed, QToolButton:pressed { background: #424852; }
QPushButton:disabled, QToolButton:disabled { color: #969ca5; background: #23252a; border-color: #36383d; }
QPushButton[primary=true] { background: #78dfc7; border-color: #78dfc7; color: #112c26; font-weight: 700; }
QPushButton[primary=true]:hover { background: #9cebd8; }
QPushButton[primary=true]:disabled { background: #354d48; color: #a2b6b0; border-color: #354d48; }
QPushButton[nav=true] { text-align: left; border: none; background: transparent; padding: 12px; border-radius: 6px; }
QPushButton[nav=true]:checked { background: #303b3b; color: #97ecd7; }
QPushButton[nav=true]:hover { background: #2e3136; }
QPushButton:focus, QToolButton:focus, QCheckBox:focus, QComboBox:focus, QLineEdit:focus, QSpinBox:focus, QPlainTextEdit:focus { border: 2px solid #91bfff; }
QGroupBox { border: none; border-top: 1px solid #3b3e44; margin-top: 18px; padding: 20px 0 4px; font-weight: 600; }
QGroupBox::title { subcontrol-origin: margin; left: 0; padding-right: 12px; }
QLineEdit, QSpinBox, QComboBox, QPlainTextEdit { background: #24262b; border: 1px solid #484d56; border-radius: 5px; padding: 7px; min-height: 22px; selection-background-color: #355e8d; }
QLineEdit:read-only { color: #c7cdd5; background: #212327; }
QComboBox QAbstractItemView { background: #292c31; selection-background-color: #355e8d; border: 1px solid #505762; }
QComboBox::drop-down { width: 22px; border: none; }
QTableView, QTableWidget, QListWidget { background: #212327; alternate-background-color: #282a30; border: 1px solid #3c4047; border-radius: 5px; gridline-color: #353941; selection-background-color: #335b86; selection-color: #ffffff; }
QHeaderView::section { background: #2d3036; color: #e1e6ed; border: none; border-right: 1px solid #424750; padding: 10px 8px; font-weight: 600; }
QTableView::item { padding: 5px; }
QTableView::item:selected { background: #335b86; }
QTabWidget::pane { border: none; border-top: 1px solid #3b3e44; padding-top: 14px; }
QTabBar::tab { padding: 10px 16px; color: #b6bdc6; border-bottom: 3px solid transparent; }
QTabBar::tab:selected { color: #98ead6; border-bottom: 3px solid #78dfc7; }
QTabBar::tab:hover { background: #2c2f35; }
QCheckBox { spacing: 9px; padding: 4px 0; }
QCheckBox::indicator { width: 17px; height: 17px; }
QProgressBar { background: #363a42; border: none; border-radius: 3px; min-height: 7px; max-height: 7px; }
QProgressBar::chunk { background: #78dfc7; border-radius: 3px; }
QStatusBar { background: #202125; color: #bdc4cc; border-top: 1px solid #34373e; }
QToolTip { background: #383e47; color: #ffffff; border: 1px solid #677181; padding: 8px; }
QMenu { background: #292c31; border: 1px solid #4b515c; padding: 5px; }
QMenu::item { padding: 8px 28px 8px 12px; }
QMenu::item:selected { background: #335b86; }
QScrollBar:vertical { background: #222429; width: 12px; }
QScrollBar::handle:vertical { background: #5b626e; min-height: 30px; border-radius: 5px; }
QScrollBar:horizontal { background: #222429; height: 12px; }
QScrollBar::handle:horizontal { background: #5b626e; min-width: 30px; border-radius: 5px; }
QScrollBar::add-line, QScrollBar::sub-line { width: 0; height: 0; }
QSplitter::handle { background: #383c43; }
"""

_icons = (Path(__file__).resolve().parents[1] / "assets" / "icons").as_posix()
STYLE += f"""
QComboBox::down-arrow {{ image: url('{_icons}/chevron-down.svg'); width: 16px; height: 16px; }}
QSpinBox::up-arrow {{ image: url('{_icons}/chevron-up.svg'); width: 12px; height: 12px; }}
QSpinBox::down-arrow {{ image: url('{_icons}/chevron-down.svg'); width: 12px; height: 12px; }}
QCheckBox::indicator {{ border: 1px solid #6f7987; border-radius: 3px; background: #24262b; }}
QCheckBox::indicator:checked {{ image: url('{_icons}/check.svg'); background: #335b86; border-color: #83ade1; }}
"""


def apply_theme(app):
    app.setStyle("Fusion")
    palette = QPalette()
    for role, color in {
        QPalette.ColorRole.Window: "#191a1d",
        QPalette.ColorRole.WindowText: "#e9eaec",
        QPalette.ColorRole.Base: "#212327",
        QPalette.ColorRole.AlternateBase: "#282a30",
        QPalette.ColorRole.Text: "#e9eaec",
        QPalette.ColorRole.Button: "#2b2d32",
        QPalette.ColorRole.ButtonText: "#e9eaec",
        QPalette.ColorRole.Highlight: "#335b86",
        QPalette.ColorRole.HighlightedText: "#ffffff",
        QPalette.ColorRole.ToolTipBase: "#383e47",
        QPalette.ColorRole.ToolTipText: "#ffffff",
        QPalette.ColorRole.PlaceholderText: "#a1a9b5",
    }.items():
        palette.setColor(role, QColor(color))
    app.setPalette(palette)
    app.setStyleSheet(STYLE)
