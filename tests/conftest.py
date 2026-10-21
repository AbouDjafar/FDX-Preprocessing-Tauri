from datetime import datetime

import pytest
from openpyxl import Workbook
from openpyxl.styles import Alignment

from fdx_atelier.models import ColumnMapping, DataType, FieldSpec, ProjectConfig, Role


@pytest.fixture
def project(tmp_path):
    path = tmp_path / "source.xlsx"
    workbook = Workbook()
    sheet = workbook.active
    sheet.title = "Données"
    sheet.append(["Entreprise", "Téléphone", "Montant", "Date", "NIU", "Libre", "Date signature"])
    sheet.append(
        [
            "  Entreprise A  ",
            "+237699123456/677123456",
            "1 234,50",
            "31/09/2025",
            "00123",
            "inchangé",
            datetime(2025, 8, 1),
        ]
    )
    sheet.append(
        [
            "Entreprise B",
            "+390498976845",
            1250.12345678901,
            "18 Fév. 2025",
            "M123456789012X",
            "=SUM(C2:C3)",
            datetime(2025, 4, 1),
        ]
    )
    sheet["A1"].alignment = Alignment(textRotation=90)
    sheet["C3"].number_format = "#,##0.00000000000"
    sheet.freeze_panes = "A2"
    sheet.auto_filter.ref = "A1:G3"
    other = workbook.create_sheet("Conserver")
    other["A1"] = "Autre feuille"
    other["A2"] = 12345.67
    other["A2"].number_format = "#,##0.00"
    workbook.save(path)
    workbook.close()
    fields = [
        FieldSpec(code="NOM", label="Entreprise", role=Role.COMPANY),
        FieldSpec(code="TEL", label="Téléphone", role=Role.PHONE),
        FieldSpec(code="MON", label="Montant", dtype=DataType.DECIMAL),
        FieldSpec(code="DATE", label="Date", dtype=DataType.DATE),
        FieldSpec(code="NIU", label="NIU", role=Role.IDENTIFIER),
        FieldSpec(code="SIGN", label="Date signature", dtype=DataType.DATE),
    ]
    config = ProjectConfig(
        source_path=str(path),
        original_path=str(path),
        sheet="Données",
        fields=fields,
        mappings=[
            ColumnMapping(column=col, field_code=spec.code, confirmed=True)
            for col, spec in zip([1, 2, 3, 4, 5, 7], fields)
        ],
    )
    return config
