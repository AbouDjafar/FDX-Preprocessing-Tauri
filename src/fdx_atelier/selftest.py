"""Offline verification of both the installed runtime and the native interface."""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path


def run(output: str):
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    from openpyxl import Workbook, load_workbook
    from PySide6.QtWidgets import QApplication

    from .engine import analyze
    from .exporter import export_excel
    from .models import ColumnMapping, DataType, Decision, FieldRule, FieldSpec, ProjectConfig, Role, cell_key
    from .ui.app import STYLE, MainWindow
    from .workbooks import inspect_workbook, sha256

    output = Path(output).resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fdx-selftest-") as temp:
        source = Path(temp) / "source.xlsx"
        workbook = Workbook()
        sheet = workbook.active
        sheet.title = "Données"
        sheet.append(["Entreprise", "Téléphone", "Montant", "Date"])
        sheet.append(["Atelier Cameroun", "699123456/677123456", "1 234,50", "31/09/2025"])
        sheet.append(["Entreprise B", "+390498976845", 1250.125, "18 Fév. 2025"])
        workbook.save(source)
        workbook.close()
        fields = [
            FieldSpec(code="NOM", label="Entreprise"),
            FieldSpec(code="TEL", label="Téléphone", role=Role.PHONE),
            FieldSpec(code="MON", label="Montant", dtype=DataType.DECIMAL),
            FieldSpec(code="DATE", label="Date", dtype=DataType.DATE),
        ]
        cfg = ProjectConfig(
            source_path=str(source),
            original_path=str(source),
            sheet="Données",
            fields=fields,
            mappings=[
                ColumnMapping(column=i, field_code=f.code, confirmed=True) for i, f in enumerate(fields, 1)
            ],
            rules={f.code: FieldRule() for f in fields},
        )
        result = analyze(cfg)
        assert any(f.kind == "phone_multiple" for f in result.findings)
        assert any(f.kind == "date_invalid" for f in result.findings)
        fixed = analyze(
            cfg,
            {
                cell_key(cfg.sheet, 2, 2): Decision(action="set", value="699123456"),
                cell_key(cfg.sheet, 2, 4): Decision(action="set", value="2025-09-30"),
            },
        )
        assert not fixed.findings, [f.message for f in fixed.findings]
        target = Path(temp) / "résultat.xlsx"
        digest = sha256(source)
        export_excel(cfg, fixed, target)
        assert sha256(source) == digest
        wb = load_workbook(target)
        assert wb.active["C2"].value == 1234.5
        assert wb.active["B2"].value == "699123456"
        wb.close()
        app = QApplication.instance() or QApplication([])
        app.setStyle("Fusion")
        app.setStyleSheet(STYLE)
        window = MainWindow()
        errors = []
        window.show_error = lambda error: errors.append(str(error))
        window.config = cfg
        window.metadata = inspect_workbook(source)
        window.import_page.load()
        window.show()
        screenshots = []
        for width, height in [(1360, 900), (1024, 768)]:
            window.resize(width, height)
            for step in range(6):
                window.navigate(step)
                if step >= 4:
                    window.result = analyze(window.config)
                    window.pages[step].load()
                app.processEvents()
                name = output.parent / f"interface-{width}-{step + 1}.png"
                assert window.grab().save(str(name))
                screenshots.append(str(name))
        assert not errors, errors
        window.dirty = False
        window.close()
        app.processEvents()
        report = {
            "ok": True,
            "rows": result.row_count,
            "gui_steps": 6,
            "viewports": 2,
            "screenshots": screenshots,
            "engine": True,
            "pandera": True,
            "source_unchanged": True,
            "export_verified": True,
        }
        output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        return report
