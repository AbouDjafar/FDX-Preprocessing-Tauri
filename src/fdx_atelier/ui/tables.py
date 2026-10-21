from PySide6.QtCore import QAbstractTableModel, QModelIndex, Qt

from ..models import display

CATEGORIES = {
    "change": "Modification",
    "phone_multiple": "Téléphones multiples",
    "phone_invalid": "Téléphone invalide",
    "date_invalid": "Date à confirmer",
    "chronology": "Chronologie",
    "length": "Longueur",
    "pattern": "Motif",
    "number_invalid": "Nombre",
    "required": "Obligatoire",
    "storage": "Stockage",
    "boolean_invalid": "Booléen",
    "segmentation_conflict": "Conflit de segmentation",
    "constraint": "Dictionnaire",
    "formula": "Formule conservée",
}


class ReviewModel(QAbstractTableModel):
    headers = ["État", "Ligne", "Colonne", "Champ", "Valeur source", "Valeur actuelle", "Proposition / motif"]

    def __init__(self, records=None, parent=None):
        super().__init__(parent)
        self.records = records or []

    def replace(self, records):
        self.beginResetModel()
        self.records = records
        self.endResetModel()

    def rowCount(self, parent=QModelIndex()):
        return 0 if parent.isValid() else len(self.records)

    def columnCount(self, parent=QModelIndex()):
        return len(self.headers)

    def headerData(self, section, orientation, role=Qt.ItemDataRole.DisplayRole):
        if orientation == Qt.Orientation.Horizontal and role == Qt.ItemDataRole.DisplayRole:
            return self.headers[section]
        return None

    def data(self, index, role=Qt.ItemDataRole.DisplayRole):
        if not index.isValid():
            return None
        row = self.records[index.row()]
        values = [
            CATEGORIES.get(row["kind"], row["kind"]),
            row["row"],
            row["column"],
            row["field_code"],
            display(row["original"]),
            display(row["current"]),
            ("À vérifier · " if row.get("pending") else "") + row["message"],
        ]
        if role == Qt.ItemDataRole.DisplayRole:
            return values[index.column()]
        if role == Qt.ItemDataRole.ToolTipRole:
            return str(values[index.column()])
        if role == Qt.ItemDataRole.UserRole:
            return row["kind"]
        if role == Qt.ItemDataRole.ForegroundRole and index.column() == 0:
            from PySide6.QtGui import QColor

            return QColor("#82e1bd" if row["kind"] == "change" else "#f0c578")
        return None


def result_records(result):
    records = [
        dict(
            key=c.key,
            row=c.row,
            column=c.column,
            field_code=c.field_code,
            kind="change",
            original=c.before,
            current=c.after,
            proposed=c.after,
            choices=[],
            message=c.reason,
        )
        for c in result.changes
    ]
    records.extend(
        dict(
            key=f.key,
            row=f.row,
            column=f.column,
            field_code=f.field_code,
            kind=f.kind,
            original=f.original,
            current=f.current,
            proposed=f.proposed,
            choices=f.choices,
            message=(f"{display(f.proposed)} · {f.message}" if f.proposed is not None else f.message),
        )
        for f in result.findings
    )
    return records
