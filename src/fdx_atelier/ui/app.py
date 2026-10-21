from __future__ import annotations

import sys
import tempfile
import threading
from pathlib import Path

from PySide6.QtCore import QLibraryInfo, QSignalBlocker, QStandardPaths, Qt, QThread, QTranslator, Signal
from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import (
    QApplication,
    QCheckBox,
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QMainWindow,
    QMenu,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QScrollArea,
    QStackedWidget,
    QToolButton,
    QVBoxLayout,
    QWhatsThis,
    QWidget,
)

from .. import __version__
from ..demo import create_demo
from ..dictionary import parse_dictionary, read_text
from ..engine import analyze
from ..exporter import REPORTS, export_excel
from ..models import Cancelled, Decision, FieldRule, display
from ..storage import load_profile, open_project, project_ui_state, save_profile, save_project
from ..transforms import split_phones, valid_phone
from ..workbooks import inspect_workbook
from .fonts import ensure_fonts
from .icons import icon
from .pages import ExportPage, ImportPage
from .prepare import PreparePage
from .review import ReviewPage
from .session import SessionController, decision_stamp
from .theme import apply_theme

TITLES = ["Mes fichiers", "Préparer les données", "Vérifier les résultats", "Enregistrer le résultat"]


class Job(QThread):
    succeeded = Signal(object)
    failed = Signal(object)
    progress = Signal(int, int, str)

    def __init__(self, function, parent=None):
        super().__init__(parent)
        self.function = function
        self.cancel = threading.Event()

    def run(self):
        try:
            self.succeeded.emit(self.function(self.cancel, self.progress.emit))
        except Exception as exc:
            self.failed.emit(exc)


class MainWindow(QMainWindow):
    config = property(lambda s: s.session.config, lambda s, v: setattr(s.session, "config", v))
    decisions = property(lambda s: s.session.decisions, lambda s, v: setattr(s.session, "decisions", v))
    history = property(lambda s: s.session.history, lambda s, v: setattr(s.session, "history", v))
    redo_history = property(
        lambda s: s.session.redo_history, lambda s, v: setattr(s.session, "redo_history", v)
    )
    result = property(lambda s: s.session.result, lambda s, v: setattr(s.session, "result", v))
    dirty = property(lambda s: s.session.dirty, lambda s, v: setattr(s.session, "dirty", v))
    advanced = property(lambda s: s.session.advanced)

    def __init__(self):
        super().__init__()
        ensure_fonts()
        self.session = SessionController()
        self.metadata = None
        self.dictionary_notes = []
        self.busy = False
        self.job = None
        self.step = 0
        self.project_path = None
        self._closing = False
        self._demo_folders = []
        self.setWindowTitle(f"FDX Atelier Données · {__version__}")
        self.setWindowIcon(icon("file-spreadsheet", "#78dfc7"))
        self.resize(1366, 850)
        self.setMinimumSize(800, 500)
        self.build_ui()

    def build_ui(self):
        shell = QWidget()
        shell.setObjectName("shell")
        main = QVBoxLayout(shell)
        main.setContentsMargins(0, 0, 0, 0)
        main.setSpacing(0)
        top = QWidget()
        top.setObjectName("topbar")
        bar = QHBoxLayout(top)
        bar.setContentsMargins(20, 12, 20, 12)
        brand = QLabel("FDX Atelier")
        brand.setObjectName("brand")
        bar.addWidget(brand)
        version = QLabel(f"DONNÉES  /  {__version__}")
        version.setObjectName("eyebrow")
        bar.addWidget(version)
        bar.addStretch()
        menu_button = QToolButton()
        menu_button.setText("Projet")
        menu_button.setIcon(icon("folder"))
        menu_button.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextBesideIcon)
        menu_button.setPopupMode(QToolButton.ToolButtonPopupMode.InstantPopup)
        menu = QMenu(menu_button)
        for text, symbol, callback in [
            ("Nouveau projet", "plus", self.new_project),
            ("Ouvrir un projet", "folder-open", self.open_session),
            ("Enregistrer le projet", "save", self.save_session),
            ("Charger un profil", "download", self.import_profile),
            ("Enregistrer un profil", "upload", self.export_profile),
        ]:
            action = QAction(icon(symbol), text, self)
            action.triggered.connect(callback)
            menu.addAction(action)
        menu_button.setMenu(menu)
        bar.addWidget(menu_button)
        self.project_menu = menu_button
        self.advanced_toggle = QCheckBox("Mode avancé")
        self.advanced_toggle.setObjectName("advancedMode")
        self.advanced_toggle.setToolTip("Afficher le dictionnaire complet et les corrections groupées.")
        self.advanced_toggle.toggled.connect(self.set_advanced)
        bar.addWidget(self.advanced_toggle)
        help_button = QToolButton()
        help_button.setIcon(icon("circle-question-mark"))
        help_button.setAccessibleName("Aide contextuelle")
        help_button.setToolTip("Aide sur un champ ou une commande")
        help_button.clicked.connect(QWhatsThis.enterWhatsThisMode)
        bar.addWidget(help_button)
        main.addWidget(top)
        center = QHBoxLayout()
        center.setSpacing(0)
        self.sidebar = QWidget()
        self.sidebar.setObjectName("navigation")
        self.sidebar.setFixedWidth(196)
        navigation = QVBoxLayout(self.sidebar)
        navigation.setContentsMargins(12, 24, 12, 18)
        self.nav_buttons = []
        for index, (text, symbol) in enumerate(
            zip(
                ["Mes fichiers", "Préparer", "Vérifier", "Enregistrer"],
                ["file-spreadsheet", "settings-2", "list-checks", "download"],
                strict=True,
            )
        ):
            b = QPushButton(f"{index + 1}   {text}")
            b.setIcon(icon(symbol))
            b.setProperty("nav", True)
            b.setCheckable(True)
            b.setAccessibleName(f"Étape {index + 1} : {text}")
            b.setToolTip(text)
            b.clicked.connect(lambda checked=False, i=index: self.navigate(i))
            navigation.addWidget(b)
            self.nav_buttons.append(b)
        navigation.addStretch()
        local = QLabel("FDX  ·  LOCAL")
        self.local_label = local
        local.setProperty("muted", True)
        navigation.addWidget(local)
        center.addWidget(self.sidebar)
        body = QVBoxLayout()
        body.setContentsMargins(28, 20, 28, 18)
        self.step_label = QLabel("ÉTAPE 1 SUR 4")
        self.step_label.setObjectName("stepNumber")
        body.addWidget(self.step_label)
        self.title = QLabel(TITLES[0])
        self.title.setObjectName("pageTitle")
        self.title.setWordWrap(True)
        body.addWidget(self.title)
        self.file_context = QLabel("Nouveau projet")
        self.file_context.setProperty("muted", True)
        self.file_context.setWordWrap(True)
        body.addWidget(self.file_context)
        self.stack = QStackedWidget()
        self.import_page = ImportPage(self)
        self.prepare_page = PreparePage(self)
        self.mapping_page = self.prepare_page.mapping
        self.dictionary_page = self.prepare_page.dictionary
        self.rules_page = self.prepare_page.rules
        self.review_page = ReviewPage(self)
        self.export_page = ExportPage(self)
        self.export_page.export_button.hide()
        self.pages = [self.import_page, self.prepare_page, self.review_page, self.export_page]
        for page in self.pages:
            page.setObjectName("page")
            scroll = QScrollArea()
            scroll.setWidgetResizable(True)
            scroll.setFrameShape(QScrollArea.Shape.NoFrame)
            scroll.setWidget(page)
            self.stack.addWidget(scroll)
        body.addWidget(self.stack, 1)
        self.progress = QProgressBar()
        self.progress.setTextVisible(False)
        self.progress.hide()
        body.addWidget(self.progress)
        footer = QHBoxLayout()
        self.previous_button = QPushButton("Retour")
        self.previous_button.setIcon(icon("arrow-left"))
        self.previous_button.clicked.connect(lambda: self.navigate(self.step - 1))
        footer.addWidget(self.previous_button)
        footer.addStretch()
        self.cancel_button = QPushButton("Annuler le traitement")
        self.cancel_button.setIcon(icon("x"))
        self.cancel_button.clicked.connect(self.cancel_job)
        self.cancel_button.hide()
        footer.addWidget(self.cancel_button)
        self.next_button = QPushButton()
        self.next_button.setObjectName("primaryAction")
        self.next_button.setProperty("primary", True)
        self.next_button.setIcon(icon("arrow-right", "#112c26"))
        self.next_button.clicked.connect(self.next_action)
        footer.addWidget(self.next_button)
        body.addLayout(footer)
        center.addLayout(body, 1)
        main.addLayout(center, 1)
        self.setCentralWidget(shell)
        for label, key, callback in [
            ("Annuler la décision", QKeySequence.StandardKey.Undo, self.undo_decision),
            ("Rétablir la décision", QKeySequence.StandardKey.Redo, self.redo_decision),
            ("Enregistrer le projet", QKeySequence.StandardKey.Save, self.save_session),
        ]:
            action = QAction(label, self)
            action.setShortcut(key)
            action.triggered.connect(callback)
            self.addAction(action)
        for widget in shell.findChildren(QWidget):
            if isinstance(widget, QLabel):
                widget.setTextFormat(Qt.TextFormat.PlainText)
            if widget.toolTip():
                widget.setWhatsThis(widget.toolTip())
        self.update_navigation()

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if hasattr(self, "sidebar"):
            compact = self.width() < 1100
            self.sidebar.setFixedWidth(72 if compact else 196)
            for i, button in enumerate(self.nav_buttons):
                button.setText(
                    ""
                    if compact
                    else f"{i + 1}   {['Mes fichiers', 'Préparer', 'Vérifier', 'Enregistrer'][i]}"
                )
            self.local_label.setText("FDX" if compact else "FDX  ·  LOCAL")

    def sheet_info(self):
        return next(
            (s for s in (self.metadata or {}).get("sheets", []) if s["name"] == self.config.sheet), None
        )

    def collect_current(self):
        before = self.config.fingerprint()
        self.pages[self.step].collect()
        if self.config.fingerprint() != before:
            self.invalidate()

    def invalidate(self):
        self.result = None
        self.session.analyzed_stamp = ""
        self.dirty = True
        if hasattr(self, "export_page"):
            self.export_page.published_path = None
            self.export_page.output.clear()
            self.review_page.load()
            self.update_navigation()

    def update_navigation(self):
        if not hasattr(self, "next_button"):
            return
        confirmed = any(m.field_code for m in self.config.mappings) and all(
            m.confirmed for m in self.config.mappings if m.field_code
        )
        text = [
            "Confirmer mes fichiers",
            "Analyser le fichier" if confirmed else "Confirmer les colonnes",
            "Choisir le fichier de sortie" if self.session.current else "Vérifier mes corrections",
            "Ouvrir mon fichier" if self.export_page.published_path else "Créer mon fichier Excel",
        ][self.step]
        self.next_button.setText(text)
        ready = bool(self.config.source_path and self.config.fields and self.metadata)
        self.next_button.setEnabled(not self.busy and (self.step != 0 or ready))
        if self.step == 3:
            self.next_button.setEnabled(not self.busy and self.session.current)
        self.next_button.setToolTip(
            "Choisir le classeur et son dictionnaire pour continuer."
            if self.step == 0 and not ready
            else text
        )
        self.next_button.setWhatsThis(self.next_button.toolTip())
        self.previous_button.setEnabled(self.step > 0 and not self.busy)
        for i, button in enumerate(self.nav_buttons):
            button.setChecked(i == self.step)
            button.setIcon(
                icon(
                    "circle-check"
                    if i < self.step
                    else ["file-spreadsheet", "settings-2", "list-checks", "download"][i],
                    "#78dfc7" if i <= self.step else "#bbc3cf",
                )
            )
            button.setEnabled(not self.busy and (i == 0 or ready))
            state = "Terminée" if i < self.step else "En cours" if i == self.step else "À venir"
            button.setToolTip(f"{TITLES[i]} · {state}")
        self.title.setText(TITLES[self.step])
        self.step_label.setText(f"ÉTAPE {self.step + 1} SUR 4")
        path = self.config.original_path or self.config.source_path
        self.file_context.setText(
            (Path(path).name + (f"  /  {self.config.sheet}" if self.config.sheet else ""))
            if path
            else "Nouveau projet"
        )
        self.statusBar().showMessage(
            "Traitement en cours"
            if self.busy
            else "Décisions à vérifier"
            if self.result and not self.session.current
            else "Données conservées sur ce poste"
        )

    def navigate(self, index):
        if not 0 <= index < 4 or self.busy:
            return
        try:
            self.collect_current()
            if index > 0 and (not self.metadata or not self.config.fields):
                raise ValueError("Choisissez le fichier Excel et son dictionnaire avant de continuer.")
            if index >= 2:
                self.config.validate_ready()
                if self.result is None:
                    self.run_analysis(lambda: self.navigate(index))
                    return
            if index == 3 and not self.session.current:
                raise ValueError("Vérifiez vos dernières corrections avant de créer le fichier.")
            self.step = index
            self.stack.setCurrentIndex(index)
            self.pages[index].load()
            self.update_navigation()
        except Exception as exc:
            self.show_error(exc)

    def next_action(self):
        if self.busy:
            return
        try:
            self.collect_current()
            if self.step == 0:
                self.navigate(1)
            elif self.step == 1:
                if not self.config.mappings or any(
                    not m.confirmed for m in self.config.mappings if m.field_code
                ):
                    self.prepare_page.confirm()
                    self.prepare_page.tabs.setCurrentIndex(1)
                    self.update_navigation()
                else:
                    self.run_analysis(lambda: self.navigate(2))
            elif self.step == 2:
                if self.session.current:
                    self.navigate(3)
                else:
                    self.run_analysis()
            elif self.export_page.published_path:
                self.export_page.open_result()
            else:
                self.run_export()
        except Exception as exc:
            self.show_error(exc)

    def set_advanced(self, enabled):
        try:
            self.collect_current()
            self.session.advanced = enabled
            self.prepare_page.set_advanced(enabled)
            self.review_page.load()
            self.dirty = True
        except Exception as exc:
            with QSignalBlocker(self.advanced_toggle):
                self.advanced_toggle.setChecked(self.advanced)
            self.show_error(exc)

    def set_dictionary(self, path):
        try:
            parsed = parse_dictionary(read_text(path))
            if not parsed.fields:
                raise ValueError(
                    "Aucun champ reconnu dans ce dictionnaire. Choisissez un tableau .md ou .txt contenant les colonnes attendues."
                )
            self.config.dictionary_path = str(Path(path).resolve())
            self.config.fields = parsed.fields
            self.config.mappings = []
            self.config.rules = {}
            self.decisions, self.history, self.redo_history = {}, [], []
            self.dictionary_notes = parsed.notes
            self.invalidate()
            self.import_page.load()
        except Exception as exc:
            self.show_error(exc)

    def set_source(self, path):
        if self.busy:
            return
        self.config.source_path = str(Path(path).resolve())
        self.config.original_path = self.config.source_path
        self.config.sheet = ""
        self.config.header_row = 1
        self.config.mappings = []
        self.decisions, self.history, self.redo_history = {}, [], []
        self.metadata = None
        self.invalidate()
        self.export_page.path.clear()
        self.refresh_source(suggest=True)

    def refresh_source(self, callback=None, suggest=False):
        path, header = self.config.source_path, self.config.header_row

        def success(metadata):
            self.metadata = metadata
            names = [s["name"] for s in metadata["sheets"]]
            if self.config.sheet not in names:
                self.config.sheet = metadata.get("suggested_sheet", names[0])
            suggestion = self.sheet_info().get("suggested_header", header)
            if suggest and suggestion != header:
                self.config.header_row = suggestion
                self.refresh_source(callback)
                return
            self.import_page.load()
            self.update_navigation()
            if callback:
                callback()

        self.run_job(lambda cancel, progress: inspect_workbook(path, header, cancel), success)

    def run_job(self, function, success):
        if self.busy:
            return
        self.busy = True
        self.stack.setEnabled(False)
        self.project_menu.setEnabled(False)
        self.advanced_toggle.setEnabled(False)
        self.cancel_button.show()
        self.progress.setRange(0, 0)
        self.progress.show()
        self.update_navigation()
        job = Job(function, self)
        self.job = job
        pending = {}
        job.succeeded.connect(lambda value: pending.update(result=value))
        job.failed.connect(lambda error: pending.update(error=error))
        job.progress.connect(self.show_progress)

        def finished():
            self.busy = False
            self.stack.setEnabled(True)
            self.project_menu.setEnabled(True)
            self.advanced_toggle.setEnabled(True)
            self.cancel_button.hide()
            self.progress.hide()
            self.job = None
            job.deleteLater()
            self.update_navigation()
            if self._closing:
                self._closing = False
                self.close()
                return
            if "error" in pending:
                if isinstance(pending["error"], Cancelled):
                    self.statusBar().showMessage("Traitement annulé · aucun fichier final créé")
                else:
                    self.show_error(pending["error"])
            elif "result" in pending:
                success(pending["result"])

        job.finished.connect(finished)
        job.start()

    def show_progress(self, value, total, message):
        self.progress.setRange(0, max(total, 1))
        self.progress.setValue(value)
        self.statusBar().showMessage(f"{message} · {value} / {total}" if total > 1 else message)

    def cancel_job(self):
        if self.job:
            self.job.cancel.set()
            self.statusBar().showMessage("Annulation demandée")

    def run_analysis(self, callback=None):
        try:
            self.collect_current()
            self.config.validate_ready()
            config = self.config.model_copy(deep=True)
            decisions = {k: v.model_copy(deep=True) for k, v in self.decisions.items()}
            stamp = decision_stamp(decisions)

            def success(result):
                if not self.session.accept(result, stamp):
                    self.show_error("Les paramètres ont changé pendant l'analyse. Relancez la vérification.")
                    return
                self.review_page.load()
                self.export_page.load()
                self.update_navigation()
                if callback:
                    callback()

            self.run_job(lambda cancel, progress: analyze(config, decisions, cancel, progress), success)
        except Exception as exc:
            self.show_error(exc)

    def apply_selected(self):
        if not self.result or self.busy:
            return
        records = {r["key"]: r for r in self.review_page.selected_records()}
        if not records:
            return
        action = self.review_page.action.currentData()
        updated = dict(self.decisions)
        for key, record in records.items():
            from .review import applicable

            if action not in applicable(record):
                continue
            rule = self.config.rules.get(record["field_code"], FieldRule())
            spec = next((f for f in self.config.fields if f.code == record["field_code"]), None)
            phones = split_phones(record["original"])
            if action == "keep":
                updated[key] = Decision(action="keep", reason="Valeur d'origine conservée")
                continue
            if action == "clear":
                updated[key] = Decision(action="clear", reason="Cellule vidée explicitement")
                continue
            if action == "first":
                value = phones[0] if phones else None
            elif action == "first_valid":
                value = next((p for p in phones if valid_phone(p, rule.phone_region)), None)
            elif action == "all":
                value = self.review_page.separator.currentData().join(phones)
            elif action == "choose":
                value = self.review_page.choice.currentData()
                if len(records) != 1:
                    self.show_error("Sélectionnez une seule cellule pour choisir une valeur.")
                    return
            elif action == "edit":
                value = self.review_page.edit.text()
            elif action == "truncate":
                value = display(record["current"])[: spec.max_length] if spec and spec.max_length else None
            else:
                value = record["proposed"]
            if value is None:
                continue
            updated[key] = Decision(
                action="set",
                value=value,
                allow_multiple=action == "all",
                reason=f"Décision utilisateur : {self.review_page.action.currentText()}",
            )
        if self.session.queue(updated, self.review_page.action.currentText()):
            self.export_page.published_path = None
            self.review_page.load()
            self.export_page.load()
            self.update_navigation()

    def undo_decision(self):
        if not self.busy and self.session.undo():
            self.review_page.load()
            self.export_page.load()
            self.update_navigation()

    def redo_decision(self):
        if not self.busy and self.session.redo():
            self.review_page.load()
            self.export_page.load()
            self.update_navigation()

    def run_export(self):
        try:
            self.collect_current()
            if not self.session.current:
                raise ValueError("Vérifiez les dernières corrections avant de créer le fichier Excel.")
            destination = Path(self.export_page.path.text())
            if not self.export_page.path.text().strip():
                raise ValueError("Choisissez le nom et l'emplacement du fichier de sortie.")
            if self.result.findings and not self.export_page.allow_issues.isChecked():
                raise ValueError(
                    "Des problèmes restent signalés. Corrigez-les ou confirmez leur conservation dans le fichier."
                )
            overwrite = False
            if destination.exists():
                if (
                    QMessageBox.question(self, "Fichier existant", f"Remplacer {destination.name} ?")
                    != QMessageBox.StandardButton.Yes
                ):
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
                self.export_page.published_path = str(path)
                self.export_page.output.setText(f"Fichier créé et vérifié : {Path(path).name}")
                self.export_page.show_open_buttons()
                self.update_navigation()

            self.run_job(
                lambda cancel, progress: export_excel(
                    config, result, destination, allow, overwrite, cancel, progress
                ),
                success,
            )
        except Exception as exc:
            self.show_error(exc)

    def show_error(self, error):
        details = str(error)
        if isinstance(error, PermissionError):
            message = "Le fichier est ouvert dans une autre application ou le dossier n'est pas accessible."
            remedy = "Fermez le fichier dans Excel, ou choisissez un autre emplacement."
        elif isinstance(error, FileNotFoundError):
            message = "Le fichier sélectionné n'est plus disponible."
            remedy = "Choisissez à nouveau son emplacement."
        elif len(details) > 450 or "validation error" in details.lower():
            message = "Certains paramètres ne sont pas valides."
            remedy = "Vérifiez les champs modifiés et les formats attendus."
        else:
            message = details
            remedy = ""
        box = QMessageBox(self)
        box.setWindowTitle("Un point à corriger")
        box.setTextFormat(Qt.TextFormat.PlainText)
        box.setIcon(QMessageBox.Icon.Warning)
        box.setText(message)
        if remedy:
            box.setInformativeText(remedy)
        box.setDetailedText(f"{type(error).__name__}: {details}")
        box.exec()

    def load_demo(self):
        if self.busy or not self.may_discard():
            return
        self.reset_session()
        folder = tempfile.mkdtemp(prefix="fdx-exemple-")
        self._demo_folders.append(folder)
        source, dictionary = create_demo(folder)
        self.set_dictionary(dictionary)
        self.set_source(source)

    def reset_session(self):
        advanced = self.advanced
        self.session = SessionController(advanced=advanced)
        self.metadata, self.project_path = None, None
        self.dictionary_notes = []
        self.step = 0
        self.stack.setCurrentIndex(0)
        self.prepare_page.active = 0
        with QSignalBlocker(self.prepare_page.tabs):
            self.prepare_page.tabs.setCurrentIndex(0)
        self.export_page.path.clear()
        self.export_page.published_path = None
        self.export_page.output.clear()
        self.import_page.load()
        self.review_page.load()
        self.update_navigation()

    def new_project(self):
        if not self.busy and self.may_discard():
            self.reset_session()

    def save_session(self):
        if self.busy:
            return False
        try:
            self.collect_current()
            if not self.config.source_path:
                raise ValueError("Choisissez d'abord un fichier Excel.")
            path, _ = QFileDialog.getSaveFileName(
                self, "Enregistrer le projet", self.project_path or "projet.fdxproj", "Projet FDX (*.fdxproj)"
            )
            if not path:
                return False
            if not path.lower().endswith(".fdxproj"):
                path += ".fdxproj"
            save_project(
                path,
                self.config,
                self.decisions,
                self.history,
                self.result if self.session.current else None,
                ui_state={"advanced": self.advanced, "step": self.step, "audit": self.session.audit},
            )
            self.project_path = path
            self.dirty = False
            self.statusBar().showMessage("Projet enregistré")
            return True
        except Exception as exc:
            self.show_error(exc)
            return False

    def open_session(self):
        if self.busy or not self.may_discard():
            return
        path, _ = QFileDialog.getOpenFileName(self, "Ouvrir un projet", "", "Projet FDX (*.fdxproj)")
        if not path:
            return
        try:
            cache = (
                Path(QStandardPaths.writableLocation(QStandardPaths.StandardLocation.AppLocalDataLocation))
                / "sessions"
            )
            config, decisions, history = open_project(path, cache)
            state = project_ui_state(path)
            self.reset_session()
            self.session = SessionController(
                config=config,
                decisions=decisions,
                history=history,
                advanced=bool(state.get("advanced", False)),
                audit=state.get("audit", []),
            )
            with QSignalBlocker(self.advanced_toggle):
                self.advanced_toggle.setChecked(self.advanced)
            self.prepare_page.set_advanced(self.advanced)
            self.project_path = path
            self.refresh_source(
                lambda: self.navigate(
                    2 if all(m.confirmed for m in self.config.mappings) and self.config.mappings else 1
                )
            )
        except Exception as exc:
            self.show_error(exc)

    def export_profile(self):
        try:
            self.collect_current()
            path, _ = QFileDialog.getSaveFileName(
                self, "Enregistrer un profil", "profil.json", "Profil JSON (*.json)"
            )
            if path:
                save_profile(path if path.lower().endswith(".json") else path + ".json", self.config)
                self.statusBar().showMessage("Profil enregistré")
        except Exception as exc:
            self.show_error(exc)

    def import_profile(self):
        if self.busy:
            return
        path, _ = QFileDialog.getOpenFileName(self, "Charger un profil", "", "Profil JSON (*.json)")
        if not path:
            return
        try:
            self.collect_current()
            self.config = load_profile(path, self.config)
            self.decisions, self.history, self.redo_history = {}, [], []
            self.invalidate()
            self.prepare_page.active = 0
            with QSignalBlocker(self.prepare_page.tabs):
                self.prepare_page.tabs.setCurrentIndex(0)
            self.import_page.load()
            if self.config.source_path:
                self.refresh_source(lambda: self.navigate(1))
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
        return (
            bool(self.save_session())
            if answer == QMessageBox.StandardButton.Save
            else answer == QMessageBox.StandardButton.Discard
        )

    def closeEvent(self, event):
        if self.busy:
            self._closing = True
            self.cancel_job()
            event.ignore()
        elif self._closing or self.may_discard():
            event.accept()
        else:
            event.ignore()


def launch():
    app = QApplication.instance() or QApplication(sys.argv[:1])
    app.setApplicationName("FDX Atelier Données")
    app.setOrganizationName("FDX")
    apply_theme(app)
    translator = QTranslator(app)
    if translator.load("qtbase_fr", QLibraryInfo.path(QLibraryInfo.LibraryPath.TranslationsPath)):
        app.installTranslator(translator)
        app.fdx_translator = translator
    window = MainWindow()
    window.show()
    return app.exec()
