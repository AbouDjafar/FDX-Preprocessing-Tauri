from __future__ import annotations

import json
from decimal import Decimal
from pathlib import Path

from openpyxl.utils import get_column_letter
from PySide6.QtCore import QSignalBlocker, QSortFilterProxyModel, Qt
from PySide6.QtWidgets import (
    QAbstractItemView,
    QCheckBox,
    QComboBox,
    QFileDialog,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QPlainTextEdit,
    QPushButton,
    QSpinBox,
    QStyle,
    QTableView,
    QTableWidget,
    QTableWidgetItem,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from ..dictionary import parse_dictionary, read_text, suggest_mappings
from ..models import (
    ChronologyRule,
    ColumnMapping,
    DataType,
    FieldRule,
    FieldSpec,
    Role,
    SegmentationRule,
    display,
)
from .tables import CATEGORIES, ReviewModel, result_records

TYPE_LABELS = {
    DataType.TEXT: "Texte",
    DataType.INTEGER: "Entier",
    DataType.DECIMAL: "Décimal",
    DataType.DATE: "Date",
    DataType.BOOLEAN: "Booléen",
}
ROLE_LABELS = {
    Role.STANDARD: "Standard",
    Role.PHONE: "Téléphone",
    Role.IDENTIFIER: "Identifiant",
    Role.COMPANY: "Entreprise",
}


def combo(options, selected=None):
    box = QComboBox()
    for value, label in options.items():
        box.addItem(label, value)
    idx = box.findData(selected)
    if idx >= 0:
        box.setCurrentIndex(idx)
    return box


def item(text, editable=True):
    result = QTableWidgetItem(str(text) if text is not None else "")
    if not editable:
        result.setFlags(result.flags() & ~Qt.ItemFlag.ItemIsEditable)
    return result


def button(text, callback, name=""):
    result = QPushButton(text)
    result.setObjectName(name)
    result.clicked.connect(callback)
    return result


def icon_button(label, icon, callback):
    result = QToolButton()
    result.setIcon(result.style().standardIcon(icon))
    result.setToolTip(label)
    result.setAccessibleName(label)
    result.setFixedSize(38, 38)
    result.clicked.connect(callback)
    return result


def setup_table(table):
    table.setAlternatingRowColors(True)
    table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
    table.verticalHeader().setVisible(False)
    table.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeMode.Interactive)
    table.horizontalHeader().setStretchLastSection(True)
    table.setWordWrap(False)


class ImportPage(QWidget):
    def __init__(self, app):
        super().__init__()
        self.app = app
        layout = QVBoxLayout(self)
        group = QGroupBox("Fichiers")
        form = QFormLayout(group)
        self.source = QLineEdit()
        self.source.setReadOnly(True)
        self.source.setObjectName("sourcePath")
        self.dictionary = QLineEdit()
        self.dictionary.setReadOnly(True)
        form.addRow("Classeur Excel", self.file_row(self.source, self.browse_source))
        form.addRow("Dictionnaire", self.file_row(self.dictionary, self.browse_dictionary))
        self.sheet = QComboBox()
        self.sheet.setObjectName("sheetSelector")
        self.header = QSpinBox()
        self.header.setRange(1, 1000)
        self.header.setKeyboardTracking(False)
        self.header.setObjectName("headerRow")
        form.addRow("Feuille de données", self.sheet)
        form.addRow("Ligne d'en-tête", self.header)
        layout.addWidget(group)
        self.summary = QLabel("Aucun classeur sélectionné")
        self.summary.setObjectName("sourceSummary")
        layout.addWidget(self.summary)
        self.preview = QTableWidget()
        self.preview.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        setup_table(self.preview)
        layout.addWidget(self.preview, 1)
        self.sheet.currentIndexChanged.connect(self.show_sheet)
        self.header.valueChanged.connect(self.header_changed)

    @staticmethod
    def file_row(line, callback):
        widget = QWidget()
        row = QHBoxLayout(widget)
        row.setContentsMargins(0, 0, 0, 0)
        row.addWidget(line, 1)
        row.addWidget(button("Parcourir…", callback))
        return widget

    def browse_source(self):
        path, _ = QFileDialog.getOpenFileName(self, "Ouvrir le classeur", "", "Classeur Excel (*.xlsx)")
        if path:
            self.app.set_source(path)

    def browse_dictionary(self):
        path, _ = QFileDialog.getOpenFileName(self, "Ouvrir le dictionnaire", "", "Dictionnaire (*.md *.txt)")
        if path:
            try:
                parsed = parse_dictionary(read_text(path))
                self.app.config.dictionary_path = path
                self.app.config.fields = parsed.fields
                self.app.dictionary_notes = parsed.notes
                self.app.config.mappings = []
                self.dictionary.setText(path)
                self.app.invalidate()
                self.app.dictionary_page.load()
                self.app.statusBar().showMessage(f"{len(parsed.fields)} champs proposés ; à confirmer")
            except Exception as exc:
                self.app.show_error(exc)

    def load(self):
        self.source.setText(self.app.config.original_path or self.app.config.source_path)
        self.dictionary.setText(self.app.config.dictionary_path)
        with QSignalBlocker(self.header):
            self.header.setValue(self.app.config.header_row)
        with QSignalBlocker(self.sheet):
            self.sheet.clear()
            for sheet in (self.app.metadata or {}).get("sheets", []):
                self.sheet.addItem(sheet["name"])
            self.sheet.setCurrentText(self.app.config.sheet)
        self.show_sheet()

    def header_changed(self):
        if self.app.config.source_path and not self.app.busy:
            self.app.config.header_row = self.header.value()
            self.app.config.mappings = []
            self.app.invalidate()
            self.app.refresh_source()

    def show_sheet(self):
        name = self.sheet.currentText()
        sheets = (self.app.metadata or {}).get("sheets", [])
        sheet = next((s for s in sheets if s["name"] == name), None)
        if sheet is None:
            return
        if name != self.app.config.sheet:
            self.app.config.sheet = name
            self.app.config.mappings = []
            self.app.invalidate()
        self.summary.setText(
            f"{sheet['rows'] - self.header.value():,} lignes de données    ·    "
            f"{sheet['columns']} colonnes".replace(",", " ")
        )
        self.preview.clear()
        self.preview.setColumnCount(sheet["columns"])
        self.preview.setHorizontalHeaderLabels(
            [f"{get_column_letter(i)} · {display(h)}" for i, h in enumerate(sheet["headers"], 1)]
        )
        self.preview.setRowCount(len(sheet["preview"]))
        for row, values in enumerate(sheet["preview"]):
            for col, value in enumerate(values):
                self.preview.setItem(row, col, item(display(value), False))
        for col in range(sheet["columns"]):
            self.preview.setColumnWidth(col, 180)

    def collect(self):
        self.app.config.header_row = self.header.value()
        self.app.config.sheet = self.sheet.currentText()


class DictionaryPage(QWidget):
    HEADERS = [
        "Code",
        "Libellé",
        "Stockage",
        "Rôle",
        "Taille max",
        "Format date",
        "Obligatoire",
        "Motif",
        "Valeurs autorisées (JSON)",
        "Minimum",
        "Maximum",
    ]

    def __init__(self, app):
        super().__init__()
        self.app = app
        layout = QVBoxLayout(self)
        tools = QHBoxLayout()
        tools.addWidget(button("Ajouter un champ", self.add))
        tools.addWidget(button("Supprimer la sélection", self.remove))
        tools.addStretch()
        layout.addLayout(tools)
        self.table = QTableWidget(0, len(self.HEADERS))
        self.table.setObjectName("dictionaryTable")
        self.table.setHorizontalHeaderLabels(self.HEADERS)
        setup_table(self.table)
        self.table.setColumnWidth(0, 165)
        self.table.setColumnWidth(1, 280)
        layout.addWidget(self.table, 1)
        self.notes = QPlainTextEdit()
        self.notes.setReadOnly(True)
        self.notes.setMaximumHeight(100)
        layout.addWidget(self.notes)

    def add(self, spec=None):
        if not isinstance(spec, FieldSpec):
            spec = FieldSpec(code=f"CHAMP_{self.table.rowCount() + 1}", label="Nouveau champ")
        row = self.table.rowCount()
        self.table.insertRow(row)
        values = [
            spec.code,
            spec.label,
            "",
            "",
            spec.max_length,
            spec.date_format,
            "",
            spec.pattern,
            json.dumps(spec.allowed_values, ensure_ascii=False) if spec.allowed_values else "",
            spec.minimum,
            spec.maximum,
        ]
        for col, value in enumerate(values):
            self.table.setItem(row, col, item(value))
        self.table.setCellWidget(row, 2, combo(TYPE_LABELS, spec.dtype))
        self.table.setCellWidget(row, 3, combo(ROLE_LABELS, spec.role))
        required = QCheckBox()
        required.setChecked(spec.required)
        self.table.setCellWidget(row, 6, required)

    def remove(self):
        for row in sorted({index.row() for index in self.table.selectedIndexes()}, reverse=True):
            self.table.removeRow(row)

    def load(self):
        self.table.setRowCount(0)
        for spec in self.app.config.fields:
            self.add(spec)
        self.notes.setPlainText("\n".join(self.app.dictionary_notes))
        self.notes.setVisible(bool(self.app.dictionary_notes))

    def collect(self):
        fields = []
        for row in range(self.table.rowCount()):

            def text(col):
                return self.table.item(row, col).text().strip()

            allowed = json.loads(text(8)) if text(8) else []
            if not isinstance(allowed, list) or any(not isinstance(v, str) for v in allowed):
                raise ValueError("Les valeurs autorisées doivent être une liste JSON de textes.")
            fields.append(
                FieldSpec(
                    code=text(0),
                    label=text(1),
                    dtype=self.table.cellWidget(row, 2).currentData(),
                    role=self.table.cellWidget(row, 3).currentData(),
                    max_length=int(text(4)) if text(4) else None,
                    date_format=text(5),
                    required=self.table.cellWidget(row, 6).isChecked(),
                    pattern=text(7),
                    allowed_values=allowed,
                    minimum=Decimal(text(9).replace(",", ".")) if text(9) else None,
                    maximum=Decimal(text(10).replace(",", ".")) if text(10) else None,
                )
            )
        if len({f.code for f in fields}) != len(fields) or any(not f.code for f in fields):
            raise ValueError("Chaque champ doit avoir un code unique non vide.")
        self.app.config.fields = fields


class MappingPage(QWidget):
    def __init__(self, app):
        super().__init__()
        self.app = app
        layout = QVBoxLayout(self)
        tools = QHBoxLayout()
        tools.addWidget(button("Recalculer les propositions", lambda: self.load(force=True)))
        tools.addWidget(button("Valider les associations", self.confirm, "confirmMappings"))
        tools.addStretch()
        layout.addLayout(tools)
        self.table = QTableWidget(0, 6)
        self.table.setObjectName("mappingTable")
        self.table.setHorizontalHeaderLabels(
            ["Validée", "Colonne", "En-tête Excel", "Exemple", "Champ du dictionnaire", "Similarité"]
        )
        setup_table(self.table)
        self.table.setColumnWidth(2, 255)
        self.table.setColumnWidth(3, 220)
        self.table.setColumnWidth(4, 310)
        layout.addWidget(self.table, 1)

    def load(self, force=False):
        sheet = self.app.sheet_info()
        if not sheet:
            return
        mappings = self.app.config.mappings
        if force or len(mappings) != sheet["columns"]:
            mappings = suggest_mappings(sheet["headers"], self.app.config.fields)
            self.app.config.mappings = mappings
        self.table.setRowCount(len(mappings))
        for row, mapping in enumerate(mappings):
            valid = QCheckBox()
            valid.setChecked(mapping.confirmed)
            self.table.setCellWidget(row, 0, valid)
            self.table.setItem(row, 1, item(f"{get_column_letter(mapping.column)} ({mapping.column})", False))
            self.table.setItem(row, 2, item(display(sheet["headers"][mapping.column - 1]), False))
            sample = next(
                (
                    display(values[mapping.column - 1])
                    for values in sheet["preview"]
                    if mapping.column <= len(values) and values[mapping.column - 1] is not None
                ),
                "",
            )
            self.table.setItem(row, 3, item(sample, False))
            box = combo(
                {None: "Ignorer", **{f.code: f"{f.code} · {f.label}" for f in self.app.config.fields}},
                mapping.field_code,
            )
            box.currentIndexChanged.connect(lambda _, check=valid: check.setChecked(False))
            self.table.setCellWidget(row, 4, box)
            self.table.setItem(row, 5, item(f"{mapping.score:.0f} %", False))

    def confirm(self):
        for row in range(self.table.rowCount()):
            self.table.cellWidget(row, 0).setChecked(bool(self.table.cellWidget(row, 4).currentData()))
        self.app.statusBar().showMessage(
            "Associations validées ; les doublons seront contrôlés avant l'analyse."
        )

    def collect(self):
        old = self.app.config.mappings
        self.app.config.mappings = [
            ColumnMapping(
                column=old[row].column,
                field_code=self.table.cellWidget(row, 4).currentData(),
                confirmed=self.table.cellWidget(row, 0).isChecked(),
                score=old[row].score,
            )
            for row in range(self.table.rowCount())
        ]


class RulesPage(QWidget):
    def __init__(self, app):
        super().__init__()
        self.app = app
        self.current_code = None
        layout = QVBoxLayout(self)
        self.field = QComboBox()
        self.field.setObjectName("ruleField")
        layout.addWidget(self.field)
        self.general = QGroupBox("Traitement du champ")
        form = QFormLayout(self.general)
        self.normalize = QCheckBox("Normaliser les espaces")
        self.convert = QCheckBox("Convertir vers le type validé")
        form.addRow(self.normalize)
        form.addRow(self.convert)
        self.length = combo(
            {
                "report": "Conserver et signaler",
                "truncate": "Tronquer à la taille maximale",
                "clear": "Vider la cellule",
            }
        )
        self.invalid = combo(
            {"report": "Conserver et signaler", "clear": "Vider si le motif n'est pas respecté"}
        )
        form.addRow("Texte trop long", self.length)
        form.addRow("Motif non conforme", self.invalid)
        self.replacements = QPlainTextEdit()
        self.replacements.setMaximumHeight(90)
        self.replacements.setToolTip('Objet JSON, par exemple {"BUDGET INVESTISSEMENT": "BIP"}')
        form.addRow("Remplacements exacts (JSON)", self.replacements)
        layout.addWidget(self.general)

        self.phone_group = QGroupBox("Téléphones")
        phone_form = QFormLayout(self.phone_group)
        self.region = QLineEdit("CM")
        self.phone_policy = combo(
            {
                "report": "Conserver et signaler",
                "first": "Garder le premier",
                "first_valid": "Garder le premier reconnu valide",
                "all": "Garder tous",
                "clear": "Vider la cellule",
            }
        )
        self.separator = QLineEdit("/")
        phone_form.addRow("Région (code pays)", self.region)
        phone_form.addRow("Plusieurs numéros", self.phone_policy)
        phone_form.addRow("Séparateur si tous conservés", self.separator)
        layout.addWidget(self.phone_group)

        self.number_group = QGroupBox("Nombres")
        numbers = QFormLayout(self.number_group)
        self.decimal_sep = combo({",": "Virgule", ".": "Point"})
        self.thousands_sep = combo({" ": "Espace", ",": "Virgule", ".": "Point", "": "Aucun"})
        self.currencies = QLineEdit("FCFA, XAF")
        numbers.addRow("Séparateur décimal en entrée", self.decimal_sep)
        numbers.addRow("Groupement en entrée", self.thousands_sep)
        numbers.addRow("Libellés monétaires à retirer", self.currencies)
        layout.addWidget(self.number_group)
        self.date_group = QGroupBox("Dates")
        date_form = QFormLayout(self.date_group)
        self.date_order = combo(
            {
                "dmy": "Jour / Mois / Année",
                "mdy": "Mois / Jour / Année",
                "ask": "Signaler les interprétations ambiguës",
            }
        )
        date_form.addRow("Interprétation des dates texte", self.date_order)
        layout.addWidget(self.date_group)

        chronology_group = QGroupBox("Chronologie")
        chrono_layout = QVBoxLayout(chronology_group)
        self.chronology = QTableWidget(0, 3)
        self.chronology.setHorizontalHeaderLabels(
            ["Active", "Champ antérieur ou égal", "Champ postérieur ou égal"]
        )
        setup_table(self.chronology)
        self.chronology.setMaximumHeight(160)
        self.chronology.setColumnWidth(1, 300)
        chrono_layout.addWidget(self.chronology)
        chrono_tools = QHBoxLayout()
        chrono_tools.addWidget(button("Ajouter une relation", self.add_chronology))
        chrono_tools.addWidget(button("Supprimer la sélection", self.remove_chronology))
        chrono_tools.addStretch()
        chrono_layout.addLayout(chrono_tools)
        layout.addWidget(chronology_group)

        self.seg_group = QGroupBox("Segmentation entreprise / BP / téléphone / localisation")
        self.seg_group.setCheckable(True)
        self.seg_group.setChecked(False)
        seg_form = QFormLayout(self.seg_group)
        self.seg_boxes = {name: QComboBox() for name in ("source", "name", "bp", "phone", "location")}
        for name, label in (
            ("source", "Colonne composite"),
            ("name", "Destination entreprise"),
            ("bp", "Destination BP"),
            ("phone", "Destination téléphone"),
            ("location", "Destination localisation"),
        ):
            seg_form.addRow(label, self.seg_boxes[name])
        self.seg_overwrite = QCheckBox("Remplacer les destinations déjà renseignées")
        seg_form.addRow(self.seg_overwrite)
        layout.addWidget(self.seg_group)
        layout.addStretch()
        self.field.currentIndexChanged.connect(self.change_field)

    def save_field(self):
        if not self.current_code:
            return
        replacements = json.loads(self.replacements.toPlainText() or "{}")
        if not isinstance(replacements, dict) or any(
            not isinstance(k, str) or not isinstance(v, str) for k, v in replacements.items()
        ):
            raise ValueError("Les remplacements doivent être un objet JSON de textes.")
        self.app.config.rules[self.current_code] = FieldRule(
            normalize_spaces=self.normalize.isChecked(),
            convert=self.convert.isChecked(),
            decimal_separator=self.decimal_sep.currentData(),
            thousands_separator=self.thousands_sep.currentData(),
            currency_tokens=[v.strip() for v in self.currencies.text().split(",") if v.strip()],
            date_order=self.date_order.currentData(),
            phone_region=self.region.text().strip().upper(),
            phone_policy=self.phone_policy.currentData(),
            phone_separator=self.separator.text(),
            length_policy=self.length.currentData(),
            invalid_policy=self.invalid.currentData(),
            replacements=replacements,
        )

    def change_field(self):
        previous = self.current_code
        try:
            self.save_field()
        except Exception as exc:
            with QSignalBlocker(self.field):
                self.field.setCurrentIndex(self.field.findData(previous))
            self.app.show_error(exc)
            return
        self.current_code = self.field.currentData()
        if not self.current_code:
            return
        rule = self.app.config.rules.get(self.current_code, FieldRule())
        self.normalize.setChecked(rule.normalize_spaces)
        self.convert.setChecked(rule.convert)
        self.length.setCurrentIndex(self.length.findData(rule.length_policy))
        self.invalid.setCurrentIndex(self.invalid.findData(rule.invalid_policy))
        self.replacements.setPlainText(json.dumps(rule.replacements, ensure_ascii=False, indent=2))
        self.region.setText(rule.phone_region)
        self.phone_policy.setCurrentIndex(self.phone_policy.findData(rule.phone_policy))
        self.separator.setText(rule.phone_separator)
        self.decimal_sep.setCurrentIndex(self.decimal_sep.findData(rule.decimal_separator))
        self.thousands_sep.setCurrentIndex(self.thousands_sep.findData(rule.thousands_separator))
        self.currencies.setText(", ".join(rule.currency_tokens))
        self.date_order.setCurrentIndex(self.date_order.findData(rule.date_order))
        spec = next((f for f in self.app.config.fields if f.code == self.current_code), None)
        self.phone_group.setVisible(bool(spec and spec.role == Role.PHONE))
        self.number_group.setVisible(bool(spec and spec.dtype in {DataType.INTEGER, DataType.DECIMAL}))
        self.date_group.setVisible(bool(spec and spec.dtype == DataType.DATE))

    def load(self):
        self.current_code = None
        with QSignalBlocker(self.field):
            self.field.clear()
            mapped = {m.field_code for m in self.app.config.mappings if m.field_code}
            for spec in self.app.config.fields:
                if spec.code in mapped:
                    self.field.addItem(f"{spec.code} · {spec.label}", spec.code)
        self.change_field()
        self.chronology.setRowCount(0)
        for rule in self.app.config.chronology:
            self.add_chronology(rule)
        seg = self.app.config.segmentation
        headers = (self.app.sheet_info() or {}).get("headers", [])
        for name, box in self.seg_boxes.items():
            box.clear()
            box.addItem("Non utilisée", None)
            for col, header in enumerate(headers, 1):
                box.addItem(f"{get_column_letter(col)} · {display(header)}", col)
            value = seg.source_column if name == "source" else getattr(seg, f"{name}_column")
            box.setCurrentIndex(max(0, box.findData(value)))
        self.seg_group.setChecked(seg.enabled)
        self.seg_overwrite.setChecked(seg.overwrite)

    def add_chronology(self, rule=None):
        if not isinstance(rule, ChronologyRule):
            dates = [f for f in self.app.config.fields if f.dtype == DataType.DATE]
            if len(dates) < 2:
                return
            rule = ChronologyRule(before=dates[0].code, after=dates[1].code)
        options = {
            f.code: f"{f.code} · {f.label}" for f in self.app.config.fields if f.dtype == DataType.DATE
        }
        row = self.chronology.rowCount()
        self.chronology.insertRow(row)
        active = QCheckBox()
        active.setChecked(rule.enabled)
        self.chronology.setCellWidget(row, 0, active)
        self.chronology.setCellWidget(row, 1, combo(options, rule.before))
        self.chronology.setCellWidget(row, 2, combo(options, rule.after))

    def remove_chronology(self):
        for row in sorted({i.row() for i in self.chronology.selectedIndexes()}, reverse=True):
            self.chronology.removeRow(row)

    def collect(self):
        self.save_field()
        self.app.config.chronology = [
            ChronologyRule(
                before=self.chronology.cellWidget(row, 1).currentData(),
                after=self.chronology.cellWidget(row, 2).currentData(),
                enabled=self.chronology.cellWidget(row, 0).isChecked(),
            )
            for row in range(self.chronology.rowCount())
        ]
        self.app.config.segmentation = SegmentationRule(
            enabled=self.seg_group.isChecked(),
            source_column=self.seg_boxes["source"].currentData() or 1,
            name_column=self.seg_boxes["name"].currentData(),
            bp_column=self.seg_boxes["bp"].currentData(),
            phone_column=self.seg_boxes["phone"].currentData(),
            location_column=self.seg_boxes["location"].currentData(),
            overwrite=self.seg_overwrite.isChecked(),
        )


class ReviewPage(QWidget):
    def __init__(self, app):
        super().__init__()
        self.app = app
        layout = QVBoxLayout(self)
        self.summary = QLabel("Analyse non exécutée")
        self.summary.setWordWrap(True)
        self.summary.setObjectName("reviewSummary")
        self.summary.setProperty("heading", True)
        layout.addWidget(self.summary)
        tools = QHBoxLayout()
        tools.addWidget(button("Analyser toutes les lignes", app.run_analysis, "analyzeButton"))
        tools.addWidget(
            icon_button("Annuler la décision", QStyle.StandardPixmap.SP_ArrowBack, app.undo_decision)
        )
        tools.addWidget(
            icon_button("Rétablir la décision", QStyle.StandardPixmap.SP_ArrowForward, app.redo_decision)
        )
        tools.addStretch()
        layout.addLayout(tools)
        filters = QHBoxLayout()
        self.search = QLineEdit()
        self.search.setPlaceholderText("Rechercher une ligne, une valeur ou un champ")
        self.category = combo({"": "Toutes les catégories", **CATEGORIES})
        filters.addWidget(self.search, 1)
        filters.addWidget(self.category)
        layout.addLayout(filters)
        self.model = ReviewModel()
        self.proxy = ReviewProxy(self)
        self.proxy.setSourceModel(self.model)
        self.proxy.setFilterCaseSensitivity(Qt.CaseSensitivity.CaseInsensitive)
        self.proxy.setFilterKeyColumn(-1)
        self.table = QTableView()
        self.table.setObjectName("reviewTable")
        self.table.setModel(self.proxy)
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.table.setSortingEnabled(True)
        self.table.setAlternatingRowColors(True)
        self.table.verticalHeader().hide()
        self.table.horizontalHeader().setStretchLastSection(True)
        for col, width in enumerate([155, 65, 75, 160, 210, 210]):
            self.table.setColumnWidth(col, width)
        layout.addWidget(self.table, 1)
        actions = QHBoxLayout()
        self.action = combo(
            {
                "proposal": "Accepter la proposition",
                "keep": "Conserver la valeur source et signaler",
                "first": "Garder le premier numéro",
                "first_valid": "Garder le premier numéro valide",
                "choose": "Choisir un numéro / une date",
                "all": "Garder tous les numéros",
                "clear": "Vider la cellule",
                "truncate": "Tronquer à la taille maximale",
                "edit": "Saisir une valeur",
            }
        )
        actions.addWidget(self.action, 1)
        actions.addWidget(button("Appliquer à la sélection", app.apply_selected, "applyDecision"))
        layout.addLayout(actions)
        self.search.textChanged.connect(self.proxy.setFilterFixedString)
        self.category.currentIndexChanged.connect(self.filter_changed)

    def filter_changed(self):
        self.proxy.category = self.category.currentData()
        self.proxy.invalidateFilter()

    def load(self):
        result = self.app.result
        if result:
            self.summary.setText(
                f"{result.row_count} lignes analysées    ·    {len(result.changes)} cellules modifiées"
                f"    ·    {len(result.findings)} anomalies restantes"
            )
            self.model.replace(result_records(result))
        else:
            self.summary.setText("Analyse à exécuter")
            self.model.replace([])

    def selected_records(self):
        rows = self.table.selectionModel().selectedRows()
        return [self.model.records[self.proxy.mapToSource(index).row()] for index in rows]

    def collect(self):
        pass


class ReviewProxy(QSortFilterProxyModel):
    category = ""

    def filterAcceptsRow(self, row, parent):
        if self.category and self.sourceModel().records[row]["kind"] != self.category:
            return False
        return super().filterAcceptsRow(row, parent)


class ExportPage(QWidget):
    def __init__(self, app):
        super().__init__()
        self.app = app
        layout = QVBoxLayout(self)
        self.summary = QLabel("Aucune analyse disponible")
        self.summary.setWordWrap(True)
        self.summary.setProperty("heading", True)
        layout.addWidget(self.summary)
        group = QGroupBox("Classeur de sortie")
        form = QFormLayout(group)
        self.path = QLineEdit()
        self.path.setObjectName("exportPath")
        form.addRow("Fichier Excel", ImportPage.file_row(self.path, self.choose_path))
        self.harmonized = QCheckBox("Ajouter Contrôle, Nom_original et À vérifier")
        self.harmonized.setChecked(True)
        form.addRow(self.harmonized)
        self.collision = combo(
            {
                "suffix": "Conserver les rapports existants et suffixer les nouveaux",
                "replace": "Remplacer les feuilles de rapport existantes",
            }
        )
        form.addRow("Noms de rapports déjà présents", self.collision)
        horizontal = QCheckBox("Orientation horizontale des cellules")
        horizontal.setChecked(True)
        horizontal.setEnabled(False)
        thousands = QCheckBox("Nombres sans séparateur de milliers")
        thousands.setChecked(True)
        thousands.setEnabled(False)
        form.addRow(horizontal)
        form.addRow(thousands)
        self.allow_issues = QCheckBox("Exporter avec les anomalies restantes signalées")
        self.allow_issues.setObjectName("allowIssues")
        form.addRow(self.allow_issues)
        layout.addWidget(group)
        self.export_button = button("Générer le classeur Excel", app.run_export, "exportButton")
        self.export_button.setProperty("primary", True)
        layout.addWidget(self.export_button)
        self.output = QLabel("")
        self.output.setWordWrap(True)
        self.output.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        layout.addWidget(self.output)
        layout.addStretch()

    def choose_path(self):
        path, _ = QFileDialog.getSaveFileName(
            self, "Enregistrer le résultat", self.path.text(), "Classeur Excel (*.xlsx)"
        )
        if path:
            self.path.setText(path if path.lower().endswith(".xlsx") else path + ".xlsx")

    def load(self):
        result = self.app.result
        self.summary.setText(
            f"{result.row_count} lignes analysées · {len(result.changes)} cellules modifiées · "
            f"{len(result.findings)} anomalies restantes"
            if result
            else "L'analyse doit être exécutée avant l'export."
        )
        if not self.path.text() and self.app.config.source_path:
            source = Path(self.app.config.original_path or self.app.config.source_path)
            self.path.setText(str(source.with_name(source.stem + " - traite.xlsx")))
        self.harmonized.setChecked(self.app.config.harmonized)
        self.collision.setCurrentIndex(self.collision.findData(self.app.config.report_collision))
        self.allow_issues.setVisible(bool(result and result.findings))
        self.export_button.setEnabled(result is not None)

    def collect(self):
        self.app.config.harmonized = self.harmonized.isChecked()
        self.app.config.report_collision = self.collision.currentData()
