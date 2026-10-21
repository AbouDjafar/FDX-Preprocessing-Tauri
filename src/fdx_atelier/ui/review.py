from collections import Counter

from PySide6.QtCore import QSortFilterProxyModel, Qt
from PySide6.QtWidgets import (
    QAbstractItemView,
    QComboBox,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QStackedWidget,
    QTabBar,
    QTableView,
    QTableWidget,
    QVBoxLayout,
    QWidget,
)

from ..models import display
from .icons import icon
from .pages import button, combo, item, setup_table
from .tables import CATEGORIES, ReviewModel, result_records

ACTIONS = {
    "proposal": "Accepter la proposition",
    "keep": "Conserver la valeur d'origine et signaler",
    "first": "Garder le premier numéro",
    "first_valid": "Garder le premier numéro valide",
    "choose": "Choisir une valeur",
    "all": "Garder tous les numéros",
    "clear": "Vider la cellule",
    "truncate": "Raccourcir à la taille autorisée",
    "edit": "Saisir une correction",
}


def applicable(record):
    if not record:
        return []
    actions = []
    if record["proposed"] is not None and record["kind"] != "change":
        actions.append("proposal")
    if record["kind"].startswith("phone_"):
        actions.extend(["first", "first_valid", "choose", "all"])
    elif record["choices"]:
        actions.append("choose")
    if record["kind"] == "length":
        actions.append("truncate")
    if record["kind"] == "formula":
        return ["keep"]
    return list(dict.fromkeys(actions + ["edit", "keep", "clear"]))


class ReviewProxy(QSortFilterProxyModel):
    category = ""
    view = 0

    def filterAcceptsRow(self, row, parent):
        record = self.sourceModel().records[row]
        if (self.view == 0) == (record["kind"] == "change"):
            return False
        if self.category and record["kind"] != self.category:
            return False
        return super().filterAcceptsRow(row, parent)


class ReviewPage(QWidget):
    def __init__(self, app):
        super().__init__()
        self.app = app
        self.position = 0
        layout = QVBoxLayout(self)
        metrics = QHBoxLayout()
        self.metrics = []
        for name in ["Lignes analysées", "Modifications préparées", "Problèmes restants"]:
            group = QVBoxLayout()
            value = QLabel("—")
            value.setObjectName("metric")
            caption = QLabel(name)
            caption.setProperty("muted", True)
            group.addWidget(value)
            group.addWidget(caption)
            metrics.addLayout(group, 1)
            self.metrics.append(value)
        layout.addLayout(metrics)
        self.summary = QLabel("Analyse non exécutée")
        self.summary.setObjectName("notice")
        self.summary.setWordWrap(True)
        layout.addWidget(self.summary)
        self.tabs = QTabBar()
        for name in ["Problèmes à traiter", "Modifications préparées", "Historique"]:
            self.tabs.addTab(name)
        layout.addWidget(self.tabs)
        filters = QHBoxLayout()
        self.search = QLineEdit()
        self.search.setPlaceholderText("Rechercher une valeur, une colonne ou une ligne")
        self.search.setAccessibleName("Rechercher dans les résultats")
        self.category = combo({"": "Tous les problèmes"})
        filters.addWidget(self.search, 1)
        filters.addWidget(self.category)
        for name, callback in [("undo-2", app.undo_decision), ("redo-2", app.redo_decision)]:
            from PySide6.QtWidgets import QToolButton

            tool = QToolButton()
            tool.setIcon(icon(name))
            tool.setToolTip("Annuler la décision" if name == "undo-2" else "Rétablir la décision")
            tool.setAccessibleName(tool.toolTip())
            tool.setFixedSize(40, 40)
            tool.clicked.connect(callback)
            filters.addWidget(tool)
        layout.addLayout(filters)
        self.model = ReviewModel()
        self.proxy = ReviewProxy(self)
        self.proxy.setSourceModel(self.model)
        self.proxy.setFilterCaseSensitivity(Qt.CaseSensitivity.CaseInsensitive)
        self.proxy.setFilterKeyColumn(-1)
        self.table = QTableView()
        self.table.setObjectName("reviewTable")
        self.table.setModel(self.proxy)
        self.table.setSortingEnabled(True)
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.table.setAlternatingRowColors(True)
        self.table.verticalHeader().hide()
        self.table.verticalHeader().setDefaultSectionSize(42)
        for col, width in enumerate([170, 65, 75, 140, 200, 200, 320]):
            self.table.setColumnWidth(col, width)
        self.guided = QWidget()
        guide = QVBoxLayout(self.guided)
        guide.setContentsMargins(0, 8, 0, 4)
        navigation = QHBoxLayout()
        self.location = QLabel()
        self.location.setProperty("heading", True)
        navigation.addWidget(self.location, 1)
        self.back_problem = button("", lambda: self.move(-1))
        self.back_problem.setIcon(icon("chevron-left"))
        self.back_problem.setAccessibleName("Problème précédent")
        self.back_problem.setToolTip("Problème précédent")
        self.next_problem = button("", lambda: self.move(1))
        self.next_problem.setIcon(icon("chevron-right"))
        self.next_problem.setAccessibleName("Problème suivant")
        self.next_problem.setToolTip("Problème suivant")
        navigation.addWidget(self.back_problem)
        navigation.addWidget(self.next_problem)
        guide.addLayout(navigation)
        self.problem = QLabel()
        self.problem.setWordWrap(True)
        guide.addWidget(self.problem)
        values = QGridLayout()
        self.before, self.after = QLabel(), QLabel()
        for col, (label, value) in enumerate(
            [("Valeur d'origine", self.before), ("Valeur proposée / préparée", self.after)]
        ):
            title = QLabel(label)
            title.setProperty("muted", True)
            values.addWidget(title, 0, col)
            value.setTextFormat(Qt.TextFormat.PlainText)
            value.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
            value.setWordWrap(True)
            value.setMinimumHeight(65)
            value.setAlignment(Qt.AlignmentFlag.AlignTop)
            value.setStyleSheet("background: #26292e; padding: 12px; border-radius: 6px;")
            values.addWidget(value, 1, col)
            values.setColumnStretch(col, 1)
        guide.addLayout(values)
        self.question = QLabel()
        self.question.setProperty("heading", True)
        guide.addWidget(self.question)
        guide.addStretch()
        self.history = QTableWidget(0, 2)
        self.history.setHorizontalHeaderLabels(["Action", "Cellules concernées"])
        setup_table(self.history)
        self.history.setColumnWidth(0, 480)
        self.history.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.content = QStackedWidget()
        for widget in [self.guided, self.table, self.history]:
            self.content.addWidget(widget)
        layout.addWidget(self.content, 1)
        self.actions_widget = QWidget()
        actions = QVBoxLayout(self.actions_widget)
        actions.setContentsMargins(0, 0, 0, 0)
        self.action = QComboBox()
        self.action.setObjectName("decisionAction")
        self.action.setAccessibleName("Décision à appliquer")
        actions.addWidget(self.action)
        self.choice = QComboBox()
        self.choice.setAccessibleName("Valeur à conserver")
        self.edit = QLineEdit()
        self.edit.setPlaceholderText("Nouvelle valeur")
        self.edit.setAccessibleName("Nouvelle valeur")
        self.separator = combo({"/": "Séparer par /", ",": "Séparer par une virgule", ";": "Séparer par ;"})
        for widget in [self.choice, self.edit, self.separator]:
            actions.addWidget(widget)
        self.apply = button("Préparer cette correction", app.apply_selected, "applyDecision")
        self.apply.setIcon(icon("check"))
        actions.addWidget(self.apply)
        layout.addWidget(self.actions_widget)
        self.search.textChanged.connect(self.filter_changed)
        self.category.currentIndexChanged.connect(self.filter_changed)
        self.tabs.currentChanged.connect(self.switch_view)
        self.table.selectionModel().selectionChanged.connect(self.selection_changed)
        self.action.currentIndexChanged.connect(self.action_changed)

    def current_record(self):
        if self.app.advanced or self.tabs.currentIndex() == 1:
            indices = self.table.selectionModel().selectedRows()
            if indices:
                return self.model.records[self.proxy.mapToSource(indices[0]).row()]
            return None
        if 0 <= self.position < self.proxy.rowCount():
            return self.model.records[self.proxy.mapToSource(self.proxy.index(self.position, 0)).row()]
        return None

    def selected_records(self):
        if self.tabs.currentIndex() == 2:
            return []
        if self.app.advanced or self.tabs.currentIndex() == 1:
            return [
                self.model.records[self.proxy.mapToSource(i).row()]
                for i in self.table.selectionModel().selectedRows()
            ]
        record = self.current_record()
        return [record] if record else []

    def filter_changed(self):
        self.proxy.category = self.category.currentData() or ""
        self.proxy.setFilterFixedString(self.search.text())
        self.proxy.invalidate()
        self.position = 0
        self.show_problem()

    def switch_view(self):
        self.proxy.view = self.tabs.currentIndex()
        self.proxy.invalidate()
        self.position = min(self.position, max(0, self.proxy.rowCount() - 1))
        self.content.setCurrentIndex(
            2
            if self.tabs.currentIndex() == 2
            else 1
            if self.app.advanced or self.tabs.currentIndex() == 1
            else 0
        )
        self.actions_widget.setVisible(self.tabs.currentIndex() != 2)
        self.category.setVisible(self.tabs.currentIndex() == 0)
        if (self.app.advanced or self.tabs.currentIndex() == 1) and self.proxy.rowCount():
            self.table.selectRow(0)
        self.show_problem()

    def move(self, delta):
        self.position = max(0, min(self.proxy.rowCount() - 1, self.position + delta))
        self.show_problem()

    def selection_changed(self):
        self.show_problem()

    def show_problem(self):
        record = self.current_record()
        count = self.proxy.rowCount()
        self.back_problem.setEnabled(self.position > 0)
        self.next_problem.setEnabled(self.position + 1 < count)
        self.action.blockSignals(True)
        self.action.clear()
        selected = self.selected_records()
        allowed = applicable(record)
        if len(selected) > 1:
            allowed = [
                action
                for action in allowed
                if action != "choose" and all(action in applicable(other) for other in selected)
            ]
        for action in allowed:
            self.action.addItem(ACTIONS[action], action)
        self.action.blockSignals(False)
        self.apply.setEnabled(bool(record))
        self.apply.setText(
            f"Préparer {len(selected)} corrections" if len(selected) > 1 else "Préparer cette correction"
        )
        if record:
            pending = record.get("pending", False)
            field = next(
                (f.label for f in self.app.config.fields if f.code == record["field_code"]),
                record["field_code"],
            )
            self.location.setText(f"{self.position + 1} / {count} · {field} · ligne {record['row']}")
            self.problem.setText(
                ("Correction en attente de vérification · " if pending else "") + record["message"]
            )
            self.before.setText(display(record["original"]))
            proposed = record["current"] if pending else record["proposed"]
            self.after.setText(display(proposed) if proposed is not None else "Aucune correction automatique")
            self.question.setText(
                "Quels numéros conserver ?"
                if record["kind"].startswith("phone_")
                else "Quelle décision prendre ?"
            )
            self.choice.clear()
            choices = record["choices"]
            if not choices and record["kind"].startswith("phone_"):
                from ..transforms import split_phones

                choices = split_phones(record["original"])
            for choice in choices:
                self.choice.addItem(display(choice), choice)
            self.edit.setText(display(record["current"]))
        else:
            self.location.setText("Aucun problème dans cette sélection")
            self.problem.setText("" if self.app.result else "Aucune analyse disponible")
            self.before.clear()
            self.after.clear()
            self.question.setText("Toutes les décisions préparées seront vérifiées avant l'export.")
        self.action_changed()

    def action_changed(self):
        action = self.action.currentData()
        self.choice.setVisible(action == "choose")
        self.edit.setVisible(action == "edit")
        self.separator.setVisible(action == "all")

    def load(self):
        result = self.app.result
        if result:
            for value, number in zip(
                self.metrics, [result.row_count, len(result.changes), len(result.findings)], strict=True
            ):
                value.setText(f"{number:,}".replace(",", " "))
            self.metrics[2].setStyleSheet("color: #f0c578;" if result.findings else "color: #82e1bd;")
            self.summary.setText(
                "Résultats à actualiser · des décisions ou paramètres ont changé"
                if not self.app.session.current
                else "Analyse terminée · toutes les lignes ont été examinées"
            )
            records = result_records(result)
            for record in records:
                decision = self.app.decisions.get(record["key"])
                if decision and not self.app.session.current:
                    record["pending"] = True
                    record["current"] = (
                        record["original"]
                        if decision.action == "keep"
                        else None
                        if decision.action == "clear"
                        else decision.value
                    )
            self.model.replace(records)
            selected = self.category.currentData()
            self.category.blockSignals(True)
            self.category.clear()
            self.category.addItem("Tous les problèmes", "")
            for kind, count in Counter(f.kind for f in result.findings).items():
                self.category.addItem(f"{CATEGORIES.get(kind, kind)} ({count})", kind)
            self.category.setCurrentIndex(max(0, self.category.findData(selected)))
            self.category.blockSignals(False)
            self.proxy.category = self.category.currentData() or ""
        else:
            for value in self.metrics:
                value.setText("—")
            self.summary.setText("Analyse à exécuter")
            self.model.replace([])
        self.history.setRowCount(len(self.app.session.audit))
        for row, entry in enumerate(self.app.session.audit):
            self.history.setItem(row, 0, item(entry["label"], False))
            self.history.setItem(row, 1, item(entry["cells"], False))
        self.position = min(self.position, max(0, self.proxy.rowCount() - 1))
        self.switch_view()

    def collect(self):
        pass
