from PySide6.QtCore import QSignalBlocker
from PySide6.QtWidgets import QLabel, QScrollArea, QTabWidget, QVBoxLayout, QWidget

from .icons import icon
from .pages import DictionaryPage, MappingPage, RulesPage


class PreparePage(QWidget):
    def __init__(self, app):
        super().__init__()
        self.app = app
        self.active = 0
        layout = QVBoxLayout(self)
        self.summary = QLabel()
        self.summary.setWordWrap(True)
        layout.addWidget(self.summary)
        self.tabs = QTabWidget()
        self.mapping = MappingPage(app)
        self.rules = RulesPage(app)
        self.dictionary = DictionaryPage(app)
        self.parts = [self.mapping, self.rules, self.dictionary]
        for page, name, symbol in zip(
            self.parts,
            ["Colonnes", "Traitements", "Dictionnaire avancé"],
            ["list-checks", "settings-2", "book-open"],
            strict=True,
        ):
            scroll = QScrollArea()
            scroll.setWidgetResizable(True)
            scroll.setFrameShape(QScrollArea.Shape.NoFrame)
            scroll.setWidget(page)
            self.tabs.addTab(scroll, icon(symbol), name)
        layout.addWidget(self.tabs, 1)
        self.tabs.currentChanged.connect(self.change_tab)

    def change_tab(self, index):
        try:
            before = self.app.config.fingerprint()
            self.parts[self.active].collect()
            if before != self.app.config.fingerprint():
                self.app.invalidate()
            self.active = index
            self.parts[index].load()
        except Exception as exc:
            with QSignalBlocker(self.tabs):
                self.tabs.setCurrentIndex(self.active)
            self.app.show_error(exc)

    def load(self):
        sheet = self.app.sheet_info() or {}
        count = len(self.app.config.fields)
        self.parts[self.active].load()
        uncertain = sum(
            bool(m.field_code) and m.score < 90 and not m.confirmed for m in self.app.config.mappings
        )
        ignored = sum(not m.field_code for m in self.app.config.mappings)
        self.summary.setText(
            f"{sheet.get('columns', 0)} colonnes Excel · {count} champs attendus · "
            f"{uncertain} correspondances incertaines · {ignored} colonnes ignorées"
            + (
                f" · {len(self.app.dictionary_notes)} notes du dictionnaire à lire en mode avancé"
                if self.app.dictionary_notes
                else ""
            )
        )
        self.set_advanced(self.app.advanced)

    def set_advanced(self, advanced):
        if not advanced and self.tabs.currentIndex() == 2:
            self.tabs.setCurrentIndex(0)
        self.tabs.setTabVisible(2, advanced)
        self.mapping.set_advanced(advanced)
        self.rules.general.layout().setRowVisible(self.rules.replacements, advanced)
        self.rules.general.layout().setRowVisible(self.rules.replacement_tools, advanced)

    def collect(self):
        self.parts[self.active].collect()

    def confirm(self):
        self.collect()
        if self.active != 0:
            self.tabs.setCurrentIndex(0)
        self.mapping.confirm()
        self.mapping.collect()
        self.app.config.validate_ready()
        self.load()
