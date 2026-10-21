from __future__ import annotations

import re
from collections import Counter
from datetime import date, datetime
from decimal import Decimal
from typing import Any

import pandas as pd
import pandera.pandas as pa

from .models import (
    AnalysisResult,
    CellChange,
    DataType,
    Decision,
    FieldRule,
    FieldSpec,
    Finding,
    ProjectConfig,
    Role,
    cell_key,
    display,
    equal_values,
)
from .transforms import clean_spaces, date_options, parse_number, segment_company, split_phones, valid_phone
from .workbooks import check_cancel, load_preserving, preflight, sha256


def blank(value: Any) -> bool:
    return value is None or isinstance(value, str) and not value.strip()


def converted(value: Any, field: FieldSpec, rule: FieldRule) -> tuple[Any, str]:
    if blank(value):
        return value, ""
    current = value
    reasons = []
    if isinstance(current, str):
        if current in rule.replacements:
            current = rule.replacements[current]
            reasons.append("Remplacement paramétré")
        if rule.normalize_spaces:
            cleaned = clean_spaces(current)
            if cleaned != current:
                reasons.append("Espaces normalisés")
            current = cleaned
    if rule.convert:
        try:
            if field.role == Role.PHONE:
                phones = split_phones(current)
                if len(phones) > 1:
                    match rule.phone_policy:
                        case "first":
                            current = phones[0]
                        case "first_valid":
                            current = next((p for p in phones if valid_phone(p, rule.phone_region)), current)
                        case "all":
                            current = rule.phone_separator.join(phones)
                        case "clear":
                            current = None
                    if rule.phone_policy != "report":
                        reasons.append("Politique de numéros multiples")
                elif phones and valid_phone(phones[0], rule.phone_region):
                    current = phones[0]
            elif field.role == Role.IDENTIFIER or field.dtype == DataType.TEXT:
                if isinstance(current, float) and current.is_integer():
                    current = str(int(current))
                else:
                    current = str(current)
            elif field.dtype in {DataType.INTEGER, DataType.DECIMAL}:
                current = parse_number(current, rule, field.dtype == DataType.INTEGER)
            elif field.dtype == DataType.DATE:
                parsed, _, _ = date_options(current, rule.date_order)
                if parsed is not None:
                    current = parsed
            elif field.dtype == DataType.BOOLEAN:
                normalized = str(current).strip().lower()
                if normalized in {"oui", "yes", "true", "1"}:
                    current = True
                elif normalized in {"non", "no", "false", "0"}:
                    current = False
        except ValueError:
            pass
    if isinstance(current, str) and field.max_length and len(current) > field.max_length:
        if rule.length_policy == "truncate":
            current = current[: field.max_length]
            reasons.append("Troncature explicitement paramétrée")
        elif rule.length_policy == "clear":
            current = None
            reasons.append("Valeur trop longue vidée")
    if current is not None and field.pattern and rule.invalid_policy == "clear":
        if not re.fullmatch(field.pattern, display(current)):
            current = None
            reasons.append("Identifiant non conforme vidé")
    if not equal_values(value, current) and not reasons:
        reasons.append("Conversion de type validée dans les paramètres")
    return current, "; ".join(reasons)


def validate_cell(
    sheet: str,
    row: int,
    col: int,
    original: Any,
    value: Any,
    spec: FieldSpec,
    rule: FieldRule,
    allow_multiple: bool = False,
) -> list[Finding]:
    issues = []

    def add(kind, message, proposed=None, choices=None):
        issues.append(
            Finding(
                key=cell_key(sheet, row, col),
                row=row,
                column=col,
                field_code=spec.code,
                kind=kind,
                message=message,
                original=original,
                current=value,
                proposed=proposed,
                choices=choices or [],
            )
        )

    if blank(value):
        if spec.required:
            add("required", "Valeur obligatoire manquante.")
        return issues
    if spec.role == Role.PHONE:
        phones = split_phones(value)
        if len(phones) > 1 and rule.phone_policy != "all" and not allow_multiple:
            add("phone_multiple", "Plusieurs numéros dans la cellule.", phones[0], phones)
        invalid = [p for p in phones if not valid_phone(p, rule.phone_region)]
        if not phones or invalid:
            add(
                "phone_invalid",
                "Numéro non reconnu pour la région choisie : " + ", ".join(invalid),
                choices=phones,
            )
    elif spec.role == Role.IDENTIFIER or spec.dtype == DataType.TEXT:
        if not isinstance(value, str):
            add("storage", "Stockage texte attendu pour préserver les identifiants.", str(value))
    elif spec.dtype == DataType.DATE and not isinstance(value, (datetime, date)):
        parsed, choices, message = date_options(value, rule.date_order)
        add(
            "date_invalid",
            message or "Conversion en date non appliquée.",
            parsed or (choices[0] if choices else None),
            choices,
        )
    elif spec.dtype in {DataType.DECIMAL, DataType.INTEGER}:
        try:
            parsed = parse_number(value, rule, spec.dtype == DataType.INTEGER)
            if isinstance(value, (str, bool)):
                add("number_invalid", "Stockage numérique attendu.", parsed)
        except ValueError as exc:
            add("number_invalid", str(exc))
    elif spec.dtype == DataType.BOOLEAN and not isinstance(value, bool):
        add("boolean_invalid", "Valeur booléenne attendue.")
    if spec.max_length and len(display(value)) > spec.max_length and not isinstance(value, (datetime, date)):
        add(
            "length",
            f"Longueur {len(display(value))} supérieure à {spec.max_length}.",
            display(value)[: spec.max_length],
        )
    if spec.pattern and not re.fullmatch(spec.pattern, display(value)):
        add("pattern", "La valeur ne respecte pas le motif du dictionnaire.")
    return issues


def manual_value(decision: Decision, raw: Any, spec: FieldSpec | None, rule: FieldRule) -> Any:
    if decision.action == "keep":
        return raw
    if decision.action == "clear":
        return None
    if decision.action != "set":
        raise ValueError(f"Décision inconnue : {decision.action}")
    value = decision.value
    if spec is None:
        return value
    # A manual decision selects an exact value; do not rerun destructive group policies on it.
    if spec.dtype == DataType.DATE and not blank(value):
        parsed, _, _ = date_options(value, rule.date_order)
        return parsed if parsed is not None else value
    if spec.dtype in {DataType.INTEGER, DataType.DECIMAL} and spec.role == Role.STANDARD and not blank(value):
        try:
            return parse_number(value, rule, spec.dtype == DataType.INTEGER)
        except ValueError:
            return value
    if spec.dtype == DataType.TEXT and value is not None:
        return str(value)
    return value


def analyze(
    config: ProjectConfig, decisions: dict[str, Decision] | None = None, cancel=None, progress=None
) -> AnalysisResult:
    config.validate_ready()
    preflight(config.source_path)
    check_cancel(cancel)
    digest = sha256(config.source_path)
    if progress:
        progress(0, 1, "Lecture du classeur")
    wb = load_preserving(config.source_path, data_only=False, rich_text=True)
    try:
        if config.sheet not in wb.sheetnames:
            raise ValueError("La feuille choisie n'existe plus.")
        ws = wb[config.sheet]
        column_count = ws.max_column
        if ws.max_row * ws.max_column > 5_000_000:
            raise ValueError("Limite de cette version : 5 millions de cellules par feuille traitée.")
        if config.header_row >= ws.max_row:
            raise ValueError("Aucune ligne de données après l'en-tête choisi.")
        if any(m.column > ws.max_column for m in config.mappings):
            raise ValueError("Une association pointe vers une colonne absente.")
        specs = {f.code: f for f in config.fields}
        by_col = {m.column: specs[m.field_code] for m in config.mappings if m.field_code}
        col_by_code = {s.code: c for c, s in by_col.items()}
        decisions = decisions or {}
        result = AnalysisResult(
            source_hash=digest,
            config_hash=config.fingerprint(),
            sheet=config.sheet,
            row_count=ws.max_row - config.header_row,
            column_count=ws.max_column,
        )
        final_values: dict[str, list[Any]] = {s.code: [] for s in by_col.values()}
        indices = []
        before_counts = Counter()
        filled, max_lengths = Counter(), Counter()
        changes = {}
        values_by_row = {}

        for row in range(config.header_row + 1, ws.max_row + 1):
            if row % 100 == 0:
                check_cancel(cancel)
                if progress:
                    progress(row - config.header_row, result.row_count, "Analyse de toutes les lignes")
            raw_values = {col: ws.cell(row, col).value for col in by_col}
            values = {}
            reasons = {}
            blocked_formulas = set()
            for col, spec in by_col.items():
                raw = raw_values[col]
                if ws.cell(row, col).data_type == "f":
                    blocked_formulas.add(col)
                    values[col] = raw
                    continue
                rule = config.rules.get(spec.code, FieldRule())
                if validate_cell(ws.title, row, col, raw, raw, spec, rule):
                    before_counts[spec.code] += 1
                values[col], reasons[col] = converted(raw, spec, rule)

            seg = config.segmentation
            if seg.enabled:
                if seg.source_column > column_count:
                    raise ValueError("Colonne de segmentation absente.")
                source = ws.cell(row, seg.source_column)
                if source.data_type != "f" and not blank(source.value):
                    parsed = segment_company(source.value)
                    result.originals.append(
                        {"sheet": ws.title, "row": row, "column": seg.source_column, "value": source.value}
                    )
                    for name, target in (
                        ("name", seg.name_column),
                        ("bp", seg.bp_column),
                        ("phone", seg.phone_column),
                        ("location", seg.location_column),
                    ):
                        if target is None or parsed[name] is None:
                            continue
                        if target > column_count:
                            raise ValueError("La segmentation doit cibler une colonne existante.")
                        key = cell_key(ws.title, row, target)
                        raw = ws.cell(row, target).value
                        raw_values.setdefault(target, raw)
                        if ws.cell(row, target).data_type == "f":
                            blocked_formulas.add(target)
                            continue
                        if (
                            target != seg.source_column
                            and not blank(raw)
                            and raw != parsed[name]
                            and not seg.overwrite
                        ):
                            if key not in decisions:
                                result.findings.append(
                                    Finding(
                                        key=key,
                                        row=row,
                                        column=target,
                                        field_code=by_col[target].code
                                        if target in by_col
                                        else f"COL_{target}",
                                        kind="segmentation_conflict",
                                        message="Destination déjà renseignée.",
                                        original=raw,
                                        current=raw,
                                        proposed=parsed[name],
                                    )
                                )
                            values.setdefault(target, raw)
                        else:
                            values[target] = parsed[name]
                            if target in by_col:
                                values[target], _ = converted(
                                    values[target],
                                    by_col[target],
                                    config.rules.get(by_col[target].code, FieldRule()),
                                )
                            reasons[target] = "Segmentation validée dans les paramètres"

            for col in list(values):
                key = cell_key(ws.title, row, col)
                spec = by_col.get(col)
                rule = config.rules.get(spec.code, FieldRule()) if spec else FieldRule()
                decision = decisions.get(key)
                if decision is not None and col not in blocked_formulas:
                    values[col] = manual_value(decision, raw_values[col], spec, rule)
                    reasons[col] = decision.reason
                if col in blocked_formulas:
                    if spec:
                        filled[spec.code] += 1
                        max_lengths[spec.code] = max(max_lengths[spec.code], len(display(values[col])))
                    result.findings.append(
                        Finding(
                            key=key,
                            row=row,
                            column=col,
                            field_code=spec.code if spec else f"COL_{col}",
                            kind="formula",
                            message="Formule conservée ; son résultat sera recalculé dans Excel.",
                            original=raw_values[col],
                            current=values[col],
                        )
                    )
                    continue
                if spec:
                    result.findings.extend(
                        validate_cell(
                            ws.title,
                            row,
                            col,
                            raw_values[col],
                            values[col],
                            spec,
                            rule,
                            decision.allow_multiple if decision else False,
                        )
                    )
                    if not blank(values[col]):
                        filled[spec.code] += 1
                        max_lengths[spec.code] = max(max_lengths[spec.code], len(display(values[col])))
                if not equal_values(raw_values[col], values[col]):
                    changes[key] = CellChange(
                        key=key,
                        row=row,
                        column=col,
                        field_code=spec.code if spec else f"COL_{col}",
                        before=raw_values[col],
                        after=values[col],
                        reason=reasons.get(col, "Décision utilisateur"),
                    )

            for code, col in col_by_code.items():
                final_values[code].append(None if col in blocked_formulas else values.get(col))
            indices.append(row)
            values_by_row[row] = values

        check_cancel(cancel)
        if progress:
            progress(result.row_count, result.row_count, "Validation du dictionnaire et de la chronologie")
        schema_columns = {}
        for code in final_values:
            spec = specs[code]
            checks = []
            if spec.allowed_values:
                checks.append(
                    pa.Check(
                        lambda v, allowed=spec.allowed_values: display(v) in allowed,
                        element_wise=True,
                        name="allowed_values",
                    )
                )
            if spec.minimum is not None:
                checks.append(
                    pa.Check(
                        lambda v, minimum=spec.minimum: _compare(v, minimum, "min"),
                        element_wise=True,
                        name="minimum",
                    )
                )
            if spec.maximum is not None:
                checks.append(
                    pa.Check(
                        lambda v, maximum=spec.maximum: _compare(v, maximum, "max"),
                        element_wise=True,
                        name="maximum",
                    )
                )
            schema_columns[code] = pa.Column(object, checks=checks, nullable=True)
        frame = pd.DataFrame(final_values, index=indices, dtype=object)
        try:
            pa.DataFrameSchema(schema_columns, strict=True, coerce=False).validate(frame, lazy=True)
        except pa.errors.SchemaErrors as exc:
            for failure in exc.failure_cases.to_dict("records"):
                if failure.get("index") is None or pd.isna(failure.get("index")):
                    raise ValueError("Le schéma de validation ne correspond pas aux données.") from exc
                row, code = int(failure["index"]), failure["column"]
                col = col_by_code[code]
                result.findings.append(
                    Finding(
                        key=cell_key(ws.title, row, col),
                        row=row,
                        column=col,
                        field_code=code,
                        kind="constraint",
                        message=f"Contrainte non satisfaite : {failure['check']}",
                        original=ws.cell(row, col).value,
                        current=values_by_row[row][col],
                    )
                )

        for row, values in values_by_row.items():
            if row % 100 == 0:
                check_cancel(cancel)
            for relation in config.chronology:
                if not relation.enabled:
                    continue
                before_col, after_col = col_by_code[relation.before], col_by_code[relation.after]
                before, after = values.get(before_col), values.get(after_col)
                if isinstance(before, (date, datetime)) and isinstance(after, (date, datetime)):
                    if display_iso(before) > display_iso(after):
                        result.findings.append(
                            Finding(
                                key=cell_key(ws.title, row, before_col),
                                row=row,
                                column=before_col,
                                field_code=relation.before,
                                kind="chronology",
                                message=f"{relation.before} ({display(before)}) après {relation.after} ({display(after)}).",
                                original=ws.cell(row, before_col).value,
                                current=before,
                            )
                        )
        result.changes = list(changes.values())
        result.counts_before = dict(before_counts)
        result.filled = dict(filled)
        result.maximum_lengths = dict(max_lengths)
        check_cancel(cancel)
        if sha256(config.source_path) != digest:
            raise ValueError("Le fichier source a changé pendant l'analyse. Relancez-la.")
        return result
    finally:
        wb.close()


def _compare(value, bound, direction):
    try:
        converted_value = Decimal(str(value))
        return converted_value >= bound if direction == "min" else converted_value <= bound
    except Exception:
        return False


def display_iso(value):
    return value.strftime("%Y-%m-%d")
