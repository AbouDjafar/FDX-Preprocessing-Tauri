"""Read original workbooks and export verification copies under qa/, never alongside sources."""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

from openpyxl import Workbook

from fdx_atelier.dictionary import parse_dictionary, read_text, suggest_mappings
from fdx_atelier.engine import analyze
from fdx_atelier.exporter import export_excel
from fdx_atelier.models import (
    ColumnMapping,
    DataType,
    FieldRule,
    FieldSpec,
    ProjectConfig,
    Role,
)
from fdx_atelier.workbooks import inspect_workbook, sha256


def run_real(root, output):
    reports = []
    for source in sorted((root / "12 Oct 2026").glob("*/Final/*.xlsx")):
        dictionary = next((source.parent.parent / "Prétraitements").glob("MAP*.md"))
        metadata = inspect_workbook(source)
        sheet = next(s for s in metadata["sheets"] if s["name"] == "Feuil2")
        fields = parse_dictionary(read_text(dictionary)).fields
        mappings = suggest_mappings(sheet["headers"], fields)
        for mapping in mappings:
            mapping.confirmed = True
        config = ProjectConfig(
            source_path=str(source),
            original_path=str(source),
            dictionary_path=str(dictionary),
            sheet="Feuil2",
            fields=fields,
            mappings=mappings,
            rules={f.code: FieldRule(phone_policy="first") for f in fields if f.role == Role.PHONE},
        )
        started = time.monotonic()
        digest = sha256(source)
        result = analyze(config)
        destination = output / (source.parent.parent.name + ".xlsx")
        export_excel(config, result, destination, allow_issues=True, overwrite=True)
        assert sha256(source) == digest
        assert result.row_count == sheet["rows"] - 1
        reports.append(
            {
                "source": str(source),
                "rows": result.row_count,
                "changes": len(result.changes),
                "issues": len(result.findings),
                "seconds": round(time.monotonic() - started, 2),
                "source_unchanged": True,
                "output": str(destination),
            }
        )
        print(json.dumps(reports[-1], ensure_ascii=False), flush=True)
    return reports


def run_large(output):
    source = output / "synthetique-source.xlsx"
    wb = Workbook(write_only=True)
    ws = wb.create_sheet("Donnees")
    ws.append(["Identifiant", "Montant", "Telephone"] + [f"Colonne {i}" for i in range(4, 26)])
    for i in range(50000):
        ws.append([f"{i:08d}", i + 0.125, "699123456/677123456"] + ["Valeur"] * 22)
    wb.save(source)
    fields = [
        FieldSpec(code="ID", label="Identifiant", role=Role.IDENTIFIER),
        FieldSpec(code="MON", label="Montant", dtype=DataType.DECIMAL),
        FieldSpec(code="TEL", label="Telephone", role=Role.PHONE),
    ]
    cfg = ProjectConfig(
        source_path=str(source),
        sheet="Donnees",
        fields=fields,
        mappings=[
            ColumnMapping(column=i, field_code=f.code, confirmed=True) for i, f in enumerate(fields, 1)
        ],
        rules={"TEL": FieldRule(phone_policy="first")},
        harmonized=False,
    )
    started = time.monotonic()
    result = analyze(cfg)
    assert result.row_count == 50000
    assert not any(f.kind == "phone_multiple" for f in result.findings)
    export_excel(cfg, result, output / "synthetique-sortie.xlsx", allow_issues=True, overwrite=True)
    report = {
        "synthetic_rows": 50000,
        "columns": 25,
        "seconds": round(time.monotonic() - started, 2),
        "changes": len(result.changes),
    }
    print(json.dumps(report), flush=True)
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--large", action="store_true")
    args = parser.parse_args()
    output = Path(__file__).resolve().parents[1] / "qa"
    output.mkdir(exist_ok=True)
    reports = run_real(args.root, output)
    if args.large:
        reports.append(run_large(output))
    (output / "recette.json").write_text(json.dumps(reports, ensure_ascii=False, indent=2), encoding="utf-8")
