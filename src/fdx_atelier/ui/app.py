from __future__ import annotations

import sys
import threading
from pathlib import Path

from PySide6.QtCore import QLibraryInfo, QStandardPaths, QThread, QTranslator, Signal
from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import (
    QApplication,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QListWidget,
    QMainWindow,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QScrollArea,
    QStackedWidget,
    QStyle,
    QToolBar,
    QVBoxLayout,
    QWidget,
)

from .. import __version__
from ..engine import analyze
from ..exporter import REPORTS, export_excel
from ..models import Cancelled, Decision, FieldRule, ProjectConfig, display
from ..storage import load_profile, open_project, save_profile, save_project
from ..transforms import split_phones, valid_phone
from ..workbooks import inspect_workbook
from .fonts import ensure_fonts
from .pages import DictionaryPage, ExportPage, ImportPage, MappingPage, ReviewPage, RulesPage

STYLE = """
QMainWindow, QScrollArea, QStackedWidget { background: #f4f6f7; }
QWidget { color: #233039; font-family: 'Segoe UI'; font-size: 10pt; }
QToolBar { background: white; border: none; border-bottom: 1px solid #d9e0e3; spacing: 10px; padding: 8px; }
QListWidget { background: #183c46; color: #e4eff0; border: none; padding: 12px 4px; }
QListWidget::item { padding: 15px 12px; margin: 3px 4px; border-radius: 4px; }
QListWidget::item:selected { background: #27606a; color: white; }
QLabel[heading=true] { font-size: 14pt; font-weight: 600; padding: 8px 0; }
QGroupBox { font-weight: 600; border: 1px solid #d4dfe3; border-radius: 5px; margin-top: 13px; padding: 15px; background: white; }
QGroupBox::title { subcontrol-origin: margin; left: 12px; padding: 0 5px; }
QLineEdit, QSpinBox, QComboBox, QPlainTextEdit { background: white; border: 1px solid #b7c8cf; border-radius: 3px; padding: 6px; min-height: 20px; }
QLineEdit:focus, QSpinBox:focus, QComboBox:focus, QPlainTextEdit:focus { border-color: #167969; }
QComboBox QAbstractItemView { background: white; color: #233039; selection-background-color: #d9eeeb; }
QPushButton { background: white; border: 1px solid #b8c9cf; padding: 8px 13px; border-radius: 4px; min-height: 19px; }
QPushButton:hover { background: #e7f0f1; border-color: #6e9a9b; }
QPushButton:disabled { color: #8a989e; background: #edf0f1; }
QPushButton[primary=true] { background: #167965; color: white; border-color: #167965; font-weight: 600; }
QTableView, QTableWidget { background: white; alternate-background-color: #f4f7f8; gridline-color: #e2e7e9; border: 1px solid #d3dde1; selection-background-color: #d8ecee; selection-color: #123d48; }
QHeaderView::section { background: #e5edf0; padding: 8px; border: none; border-right: 1px solid #cedbdf; color: #28444e; font-weight: 600; }
QStatusBar { background: white; border-top: 1px solid #d9e0e3; }
QProgressBar { background: #e6ecef; border: none; min-height: 7px; max-height: 9px; }
QProgressBar::chunk { background: #238b76; }
QCheckBox { spacing: 7px; }
"""


class Job(QThread):
    succeeded = Signal(object)
    failed = Signal(str)
    progress = Signal(int, int, str)

    def __init__(self, function, parent=None):
        super().__init__(parent)
        self.function = function
        self.cancel = threading.Event()

    def run(self):
        try:
            result = self.function(self.cancel, self.progress.emit)
            self.succeeded.emit(result)
        except Cancelled as exc:
            self.failed.emit(str(exc))
        except Exception as exc:
            self.failed.emit(f"{type(exc).__name__} : {exc}")


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        ensure_fonts()
        self.setWindowTitle(f"FDX Atelier Données · {__version__}")
        self.resize(1360, 900)
        self.setMinimumSize(1024, 680)
        self.config = ProjectConfig()
        self.decisions: dict[str, Decision] = {}
        self.history: list[dict] = []
        self.redo_history: list[dict] = []
        self.metadata = None
        self.dictionary_notes = []
        self.result = None
        self.dirty = False
        self.busy = False
        self.job = None
        self._closing = False
        self.step = 0
        self.project_path = None
        self.build_ui()

    def build_ui(self):
        bar = QToolBar("Projet")
        bar.setMovable(False)
        self.addToolBar(bar)
        actions = [
            ("Nouveau", QStyle.StandardPixmap.SP_FileIcon, self.new_project),
            ("Ouvrir un projet", QStyle.StandardPixmap.SP_DialogOpenButton, self.open_session),
            ("Enregistrer le projet", QStyle.StandardPixmap.SP_DialogSaveButton, self.save_session),
            ("Charger un profil", QStyle.StandardPixmap.SP_ArrowDown, self.import_profile),
            ("Enregistrer le profil", QStyle.StandardPixmap.SP_ArrowUp, self.export_profile),
        ]
        self.toolbar_actions = []
        for text, icon, callback in actions:
            action = QAction(self.style().standardIcon(icon), text, self)
            action.triggered.connect(callback)
            bar.addAction(action)
            self.toolbar_actions.append(action)
        central = QWidget()
        outer = QHBoxLayout(central)
        outer.setContentsMargins(0, 0, 0, 0)
        self.sidebar = QListWidget()
        self.sidebar.setObjectName("stepNavigation")
        self.sidebar.setFixedWidth(218)
        self.sidebar.addItems(
            [
                "1  Importer",
                "2  Dictionnaire",
                "3  Associations",
                "4  Paramètres",
                "5  Examiner",
                "6  Exporter",
            ]
        )
        outer.addWidget(self.sidebar)
        body = QVBoxLayout()
        body.setContentsMargins(24, 15, 24, 16)
        self.title = QLabel("Importer")
        self.title.setProperty("heading", True)
        body.addWidget(self.title)
        self.stack = QStackedWidget()
        self.import_page = ImportPage(self)
        self.dictionary_page = DictionaryPage(self)
        self.mapping_page = MappingPage(self)
        self.rules_page = RulesPage(self)
        self.review_page = ReviewPage(self)
        self.export_page = ExportPage(self)
        self.pages = [
            self.import_page,
            self.dictionary_page,
            self.mapping_page,
            self.rules_page,
            self.review_page,
            self.export_page,
        ]
        for page in self.pages:
            scroll = QScrollArea()
            scroll.setWidgetResizable(True)
            scroll.setFrameShape(QScrollArea.Shape.NoFrame)
            scroll.setWidget(page)
            self.stack.addWidget(scroll)
        body.addWidget(self.stack, 1)
        navigation = QHBoxLayout()
        self.previous_button = QPushButton("Précédent")
        self.previous_button.clicked.connect(lambda: self.navigate(self.step - 1))
        self.next_button = QPushButton("Suivant")
        self.next_button.setProperty("primary", True)
        self.next_button.clicked.connect(lambda: self.navigate(self.step + 1))
        navigation.addWidget(self.previous_button)
        navigation.addStretch()
        self.cancel_button = QPushButton("Annuler le traitement")
        self.cancel_button.clicked.connect(self.cancel_job)
        self.cancel_button.hide()
        navigation.addWidget(self.cancel_button)
        navigation.addWidget(self.next_button)
        body.addLayout(navigation)
        self.progress = QProgressBar()
        self.progress.setTextVisible(False)
        self.progress.hide()
        body.addWidget(self.progress)
        outer.addLayout(body, 1)
        self.setCentralWidget(central)
        self.sidebar.currentRowChanged.connect(self.navigate)
        self.sidebar.setCurrentRow(0)
        self.statusBar().showMessage("Nouveau projet")
        undo = QAction("Annuler la décision", self)
        undo.setShortcut(QKeySequence.StandardKey.Undo)
        undo.triggered.connect(self.undo_decision)
        self.addAction(undo)
        self.previous_button.setEnabled(False)

    def collect_current(self):
        old = self.config.fingerprint()
        self.pages[self.step].collect()
        if self.config.fingerprint() != old:
            self.invalidate()

    def navigate(self, index):
        if index < 0 or index >= 6 or self.busy:
            return
        try:
            self.collect_current()
            if index > 0 and (not self.config.source_path or not self.metadata):
                raise ValueError("Sélectionnez un classeur Excel et sa feuille de données.")
            if index >= 2 and not self.config.fields:
                raise ValueError("Importez ou renseignez les champs du dictionnaire.")
            if index == 3 and not self.config.mappings:
                self.mapping_page.load()
            self.step = index
            self.stack.setCurrentIndex(index)
            self.pages[index].load()
            self.title.setText(
                [
                    "Importer",
                    "Confirmer le dictionnaire",
                    "Associer les colonnes",
                    "Paramétrer le traitement",
                    "Examiner les résultats",
                    "Exporter le classeur",
                ][index]
            )
            self.sidebar.blockSignals(True)
            self.sidebar.setCurrentRow(index)
            self.sidebar.blockSignals(False)
            self.previous_button.setEnabled(index > 0)
            self.next_button.setVisible(index < 5)
        except Exception as exc:
            self.sidebar.blockSignals(True)
            self.sidebar.setCurrentRow(self.step)
            self.sidebar.blockSignals(False)
            self.show_error(exc)

    def sheet_info(self):
        return next(
            (s for s in (self.metadata or {}).get("sheets", []) if s["name"] == self.config.sheet), None
        )

    def invalidate(self):
        self.result = None
        self.dirty = True
        self.review_page.load()
        self.export_page.output.clear()
        self.statusBar().showMessage("Paramètres modifiés : analyse à actualiser")

    def set_source(self, path):
        self.config.source_path = str(Path(path).resolve())
        self.config.original_path = self.config.source_path
        self.config.mappings = []
        self.decisions = {}
        self.history = []
        self.redo_history = []
        self.invalidate()
        self.export_page.path.clear()
        self.refresh_source()

    def refresh_source(self, callback=None):
        path, header = self.config.source_path, self.config.header_row

        def success(metadata):
            self.metadata = metadata
            names = [s["name"] for s in metadata["sheets"]]
            if self.config.sheet not in names:
                self.config.sheet = "Feuil2" if "Feuil2" in names else names[0]
            self.import_page.load()
            if callback:
                callback()

        self.run_job(lambda cancel, progress: inspect_workbook(path, header, cancel), success)

    def run_job(self, function, success):
        if self.busy:
            return
        self.busy = True
        self.sidebar.setEnabled(False)
        self.stack.setEnabled(False)
        for action in self.toolbar_actions:
            action.setEnabled(False)
        self.previous_button.setEnabled(False)
        self.next_button.setEnabled(False)
        self.cancel_button.show()
        self.progress.setRange(0, 0)
        self.progress.show()
        job = Job(function, self)
        self.job = job
        pending = {}
        job.succeeded.connect(lambda value: pending.update(result=value))
        job.failed.connect(lambda message: pending.update(error=message))
        job.progress.connect(self.show_progress)

        def finished():
            self.busy = False
            self.sidebar.setEnabled(True)
            self.stack.setEnabled(True)
            for action in self.toolbar_actions:
                action.setEnabled(True)
            self.previous_button.setEnabled(self.step > 0)
            self.next_button.setEnabled(True)
            self.cancel_button.hide()
            self.progress.hide()
            self.job = None
            job.deleteLater()
            if self._closing:
                self.close()
                return
            if "error" in pending:
                if "annulée" in pending["error"]:
                    self.statusBar().showMessage(pending["error"])
                else:
                    self.show_error(pending["error"])
            elif "result" in pending:
                success(pending["result"])

        job.finished.connect(finished)
        job.start()

    def show_progress(self, value, total, message):
        self.progress.setRange(0, max(total, 1))
        self.progress.setValue(value)
        self.statusBar().showMessage(message)

    def cancel_job(self):
        if self.job:
            self.job.cancel.set()
            self.statusBar().showMessage("Annulation demandée")

    def run_analysis(self):
        try:
            self.collect_current()
            self.config.validate_ready()
            config = self.config.model_copy(deep=True)
            decisions = {k: v.model_copy(deep=True) for k, v in self.decisions.items()}

            def success(result):
                self.result = result
                self.review_page.load()
                self.export_page.load()
                self.statusBar().showMessage(f"Analyse terminée : {result.row_count} lignes examinées")

            self.run_job(lambda cancel, progress: analyze(config, decisions, cancel, progress), success)
        except Exception as exc:
            self.show_error(exc)

    def apply_selected(self):
        if not self.result or self.busy:
            return
        records = self.review_page.selected_records()
        unique = {r["key"]: r for r in records}
        if not unique:
            self.statusBar().showMessage("Sélectionnez les cellules concernées dans le tableau.")
            return
        action = self.review_page.action.currentData()
        if action == "choose" and len(unique) != 1:
            self.show_error("Choisissez une seule cellule pour sélectionner une interprétation.")
            return
        edited = None
        separator = "/"
        if action == "edit":
            edited, accepted = QInputDialog.getText(
                self, "Valeur de remplacement", f"Nouvelle valeur pour {len(unique)} cellule(s)"
            )
            if not accepted:
                return
        if action == "all":
            separator, accepted = QInputDialog.getText(self, "Numéros conservés", "Séparateur", text="/")
            if not accepted:
                return
            if separator not in {"/", ";", ",", " | "}:
                self.show_error("Choisissez /, ;, une virgule ou ' | ' comme séparateur.")
                return
        updated = dict(self.decisions)
        skipped = 0
        for key, record in unique.items():
            phones = split_phones(record["current"])
            spec = next((f for f in self.config.fields if f.code == record["field_code"]), None)
            rule = self.config.rules.get(record["field_code"], FieldRule())
            value = None
            if action in {"first", "first_valid", "choose", "all"} and action != "choose":
                if not spec or spec.role.value != "phone":
                    skipped += 1
                    continue
            if action == "keep":
                updated[key] = Decision(action="keep", reason="Valeur source conservée par l'utilisateur")
                continue
            if action == "clear":
                updated[key] = Decision(action="clear", reason="Cellule vidée par l'utilisateur")
                continue
            if action == "first":
                value = phones[0] if phones else None
            elif action == "first_valid":
                value = next((p for p in phones if valid_phone(p, rule.phone_region)), None)
            elif action == "all":
                value = separator.join(phones) if phones else None
            elif action == "edit":
                value = edited
            elif action == "proposal":
                value = record["proposed"]
            elif action == "truncate":
                value = display(record["current"])[: spec.max_length] if spec and spec.max_length else None
            elif action == "choose":
                choices = record["choices"] or phones
                labels = [display(c) for c in choices]
                if not labels:
                    skipped += 1
                    continue
                selected, accepted = QInputDialog.getItem(
                    self, "Choisir une valeur", "Valeur", labels, editable=False
                )
                if not accepted:
                    return
                value = choices[labels.index(selected)]
            if value is None:
                skipped += 1
                continue
            updated[key] = Decision(
                action="set",
                value=value,
                allow_multiple=action == "all",
                reason=f"Décision utilisateur : {self.review_page.action.currentText()}",
            )
        if updated != self.decisions:
            self.history.append({k: v.model_dump(mode="json") for k, v in self.decisions.items()})
            self.redo_history = []
            self.decisions = updated
            self.dirty = True
            self.run_analysis()
        elif skipped:
            self.statusBar().showMessage("Aucune action applicable à la sélection.")

    def undo_decision(self):
        if not self.history or self.busy:
            return
        self.redo_history.append({k: v.model_dump(mode="json") for k, v in self.decisions.items()})
        self.decisions = {k: Decision.model_validate(v) for k, v in self.history.pop().items()}
        self.dirty = True
        self.run_analysis()

    def redo_decision(self):
        if not self.redo_history or self.busy:
            return
        self.history.append({k: v.model_dump(mode="json") for k, v in self.decisions.items()})
        self.decisions = {k: Decision.model_validate(v) for k, v in self.redo_history.pop().items()}
        self.dirty = True
        self.run_analysis()

    def run_export(self):
        try:
            self.export_page.collect()
            if self.result is None:
                raise ValueError("L'analyse doit être exécutée avant l'export.")
            destination = Path(self.export_page.path.text())
            overwrite = False
            if destination.exists():
                answer = QMessageBox.question(self, "Fichier existant", f"Remplacer {destination.name} ?")
                if answer != QMessageBox.StandardButton.Yes:
                    return
                overwrite = True
            if self.config.harmonized and self.config.report_collision == "replace":
                collisions = [s["name"] for s in self.metadata["sheets"] if s["name"] in REPORTS]
                if (
                    collisions
                    and QMessageBox.question(
                        self, "Rapports existants", "Remplacer ces feuilles : " + ", ".join(collisions) + " ?"
                    )
                    != QMessageBox.StandardButton.Yes
                ):
                    return
            config, result = self.config.model_copy(deep=True), self.result
            allow = self.export_page.allow_issues.isChecked()

            def success(path):
                self.export_page.output.setText(f"Classeur généré et vérifié :\n{path}")
                self.statusBar().showMessage("Export terminé")

            self.run_job(
                lambda cancel, progress: export_excel(
                    config, result, destination, allow, overwrite, cancel, progress
                ),
                success,
            )
        except Exception as exc:
            self.show_error(exc)

    def show_error(self, error):
        self.statusBar().showMessage(str(error))
        QMessageBox.warning(self, "FDX Atelier Données", str(error))

    def new_project(self):
        if not self.may_discard():
            return
        self.config = ProjectConfig()
        self.decisions, self.history, self.redo_history = {}, [], []
        self.result, self.metadata, self.project_path = None, None, None
        self.dictionary_notes = []
        self.step = 0
        self.dirty = False
        self.export_page.path.clear()
        self.import_page.load()
        self.navigate(0)

    def save_session(self):
        from PySide6.QtWidgets import QFileDialog

        try:
            self.collect_current()
            if not self.config.source_path:
                raise ValueError("Choisissez d'abord un classeur.")
            path, _ = QFileDialog.getSaveFileName(
                self, "Enregistrer le projet", self.project_path or "projet.fdxproj", "Projet FDX (*.fdxproj)"
            )
            if not path:
                return False
            if not path.lower().endswith(".fdxproj"):
                path += ".fdxproj"
            save_project(path, self.config, self.decisions, self.history, self.result)
            self.project_path = path
            self.dirty = False
            self.statusBar().showMessage(f"Projet enregistré : {path}")
            return True
        except Exception as exc:
            self.show_error(exc)
            return False

    def open_session(self):
        from PySide6.QtWidgets import QFileDialog

        if not self.may_discard():
            return
        path, _ = QFileDialog.getOpenFileName(self, "Ouvrir un projet", "", "Projet FDX (*.fdxproj)")
        if not path:
            return
        try:
            cache = (
                Path(QStandardPaths.writableLocation(QStandardPaths.StandardLocation.AppLocalDataLocation))
                / "sessions"
            )
            self.config, self.decisions, self.history = open_project(path, cache)
            self.step = 0
            self.stack.setCurrentIndex(0)
            self.redo_history = []
            self.project_path = path
            self.result = None
            self.export_page.path.clear()
            self.export_page.output.clear()
            self.dirty = False
            self.refresh_source(lambda: self.navigate(4))
        except Exception as exc:
            self.show_error(exc)

    def export_profile(self):
        from PySide6.QtWidgets import QFileDialog

        try:
            self.collect_current()
            path, _ = QFileDialog.getSaveFileName(
                self, "Enregistrer le profil", "profil.json", "Profil JSON (*.json)"
            )
            if path:
                if not path.lower().endswith(".json"):
                    path += ".json"
                save_profile(path, self.config)
                self.statusBar().showMessage("Profil enregistré")
        except Exception as exc:
            self.show_error(exc)

    def import_profile(self):
        from PySide6.QtWidgets import QFileDialog

        path, _ = QFileDialog.getOpenFileName(self, "Charger un profil", "", "Profil JSON (*.json)")
        if not path:
            return
        try:
            self.collect_current()
            self.config = load_profile(path, self.config)
            self.decisions = {}
            self.history, self.redo_history = [], []
            self.invalidate()
            self.import_page.load()
            self.dictionary_page.load()
            self.mapping_page.load()
            self.rules_page.load()
            if self.config.source_path:
                self.refresh_source(lambda: self.navigate(2))
            else:
                self.navigate(0)
        except Exception as exc:
            self.show_error(exc)

    def may_discard(self):
        if not self.dirty or not self.config.source_path:
            return True
        answer = QMessageBox.question(
            self,
            "Projet en cours",
            "Enregistrer le projet avant de continuer ?",
            QMessageBox.StandardButton.Save
            | QMessageBox.StandardButton.Discard
            | QMessageBox.StandardButton.Cancel,
        )
        if answer == QMessageBox.StandardButton.Save:
            return bool(self.save_session())
        return answer == QMessageBox.StandardButton.Discard

    def closeEvent(self, event):
        if self.busy:
            self._closing = True
            self.cancel_job()
            event.ignore()
            return
        if self._closing or self.may_discard():
            event.accept()
        else:
            event.ignore()


def launch():
    app = QApplication.instance() or QApplication(sys.argv[:1])
    app.setApplicationName("FDX Atelier Données")
    app.setOrganizationName("FDX")
    app.setStyle("Fusion")
    app.setStyleSheet(STYLE)
    translator = QTranslator(app)
    if translator.load("qtbase_fr", QLibraryInfo.path(QLibraryInfo.LibraryPath.TranslationsPath)):
        app.installTranslator(translator)
        app.fdx_translator = translator
    window = MainWindow()
    window.show()
    return app.exec()
