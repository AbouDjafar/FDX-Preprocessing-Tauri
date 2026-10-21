from __future__ import annotations

import hashlib
import json
from datetime import date, datetime
from decimal import Decimal
from enum import StrEnum
from typing import Any, Callable

from pydantic import BaseModel, ConfigDict, Field, model_validator


class DataType(StrEnum):
    TEXT = "text"
    INTEGER = "integer"
    DECIMAL = "decimal"
    DATE = "date"
    BOOLEAN = "boolean"


class Role(StrEnum):
    STANDARD = "standard"
    PHONE = "phone"
    IDENTIFIER = "identifier"
    COMPANY = "company"


class FieldSpec(BaseModel):
    code: str
    label: str
    dtype: DataType = DataType.TEXT
    role: Role = Role.STANDARD
    max_length: int | None = Field(default=None, ge=1)
    date_format: str = "DD/MM/YYYY"
    required: bool = False
    pattern: str = ""
    allowed_values: list[str] = Field(default_factory=list)
    minimum: Decimal | None = None
    maximum: Decimal | None = None


class ColumnMapping(BaseModel):
    column: int = Field(ge=1)
    field_code: str | None = None
    confirmed: bool = False
    score: float = 0


class FieldRule(BaseModel):
    normalize_spaces: bool = True
    convert: bool = True
    decimal_separator: str = ","
    thousands_separator: str = " "
    currency_tokens: list[str] = Field(default_factory=lambda: ["FCFA", "XAF"])
    date_order: str = "dmy"
    phone_region: str = "CM"
    phone_policy: str = "report"
    phone_separator: str = "/"
    length_policy: str = "report"
    invalid_policy: str = "report"
    replacements: dict[str, str] = Field(default_factory=dict)


class ChronologyRule(BaseModel):
    before: str
    after: str
    enabled: bool = True


class SegmentationRule(BaseModel):
    enabled: bool = False
    source_column: int = 1
    name_column: int | None = None
    bp_column: int | None = None
    phone_column: int | None = None
    location_column: int | None = None
    overwrite: bool = False


class ProjectConfig(BaseModel):
    version: int = 1
    source_path: str = ""
    original_path: str = ""
    dictionary_path: str = ""
    sheet: str = ""
    header_row: int = Field(default=1, ge=1)
    fields: list[FieldSpec] = Field(default_factory=list)
    mappings: list[ColumnMapping] = Field(default_factory=list)
    rules: dict[str, FieldRule] = Field(default_factory=dict)
    chronology: list[ChronologyRule] = Field(default_factory=list)
    segmentation: SegmentationRule = Field(default_factory=SegmentationRule)
    harmonized: bool = True
    report_collision: str = "suffix"

    @model_validator(mode="after")
    def supported_version(self):
        if self.version != 1:
            raise ValueError("Version de profil non prise en charge")
        return self

    def validate_ready(self) -> None:
        import re

        codes = [f.code.strip() for f in self.fields]
        if not codes or any(not code for code in codes) or len(codes) != len(set(codes)):
            raise ValueError("Le dictionnaire doit contenir des codes non vides et uniques.")
        mapped = [m.field_code for m in self.mappings if m.field_code]
        if not mapped:
            raise ValueError("Associez au moins une colonne au dictionnaire.")
        if len(mapped) != len(set(mapped)):
            raise ValueError("Un champ du dictionnaire ne peut pas correspondre à deux colonnes.")
        if any(m.field_code not in codes or not m.confirmed for m in self.mappings if m.field_code):
            raise ValueError("Validez toutes les associations actives.")
        positions = [m.column for m in self.mappings]
        if len(positions) != len(set(positions)):
            raise ValueError("Une colonne Excel ne peut être associée qu'une seule fois.")
        for field in self.fields:
            if field.required and field.code not in mapped:
                raise ValueError(f"Champ obligatoire sans colonne : {field.code}")
            if field.pattern:
                re.compile(field.pattern)
            if field.minimum is not None and field.maximum is not None and field.minimum > field.maximum:
                raise ValueError(f"Bornes inversées : {field.code}")
        for rule in self.rules.values():
            if rule.decimal_separator not in {".", ","}:
                raise ValueError("Le séparateur décimal doit être un point ou une virgule.")
            if rule.decimal_separator == rule.thousands_separator:
                raise ValueError("Les séparateurs décimal et de groupement doivent être différents.")
            if rule.phone_region not in __import__("phonenumbers").SUPPORTED_REGIONS:
                raise ValueError(f"Région téléphonique inconnue : {rule.phone_region}")
            if rule.phone_policy not in {"report", "first", "first_valid", "all", "clear"}:
                raise ValueError("Politique téléphone inconnue")
            if rule.date_order not in {"dmy", "mdy", "ask"}:
                raise ValueError("Ordre de date inconnu")
        for relation in self.chronology:
            if relation.enabled and (relation.before not in mapped or relation.after not in mapped):
                raise ValueError("Une règle de chronologie utilise un champ sans colonne.")
        if self.segmentation.enabled:
            seg = self.segmentation
            targets = [seg.name_column, seg.bp_column, seg.phone_column, seg.location_column]
            active = [c for c in targets if c is not None]
            if not active or len(active) != len(set(active)):
                raise ValueError("Choisissez des colonnes de destination distinctes pour la segmentation.")

    def fingerprint(self) -> str:
        payload = self.model_dump(mode="json", exclude={"harmonized", "report_collision"})
        return hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()


class Decision(BaseModel):
    action: str = "keep"
    value: Any = None
    value_kind: str | None = None
    allow_multiple: bool = False
    reason: str = "Décision utilisateur"

    @model_validator(mode="after")
    def retain_typed_value(self):
        if self.value_kind == "decimal" and isinstance(self.value, str):
            self.value = Decimal(self.value)
        elif self.value_kind == "date" and isinstance(self.value, str):
            self.value = datetime.fromisoformat(self.value)
        if isinstance(self.value, Decimal):
            self.value_kind = "decimal"
        elif isinstance(self.value, (datetime, date)):
            self.value_kind = "date"
        return self


def cell_key(sheet: str, row: int, column: int) -> str:
    return f"{sheet}!{row}:{column}"


class Finding(BaseModel):
    key: str
    row: int
    column: int
    field_code: str
    kind: str
    message: str
    original: Any = None
    current: Any = None
    proposed: Any = None
    choices: list[Any] = Field(default_factory=list)


class CellChange(BaseModel):
    key: str
    row: int
    column: int
    field_code: str
    before: Any = None
    after: Any = None
    reason: str


class AnalysisResult(BaseModel):
    model_config = ConfigDict(arbitrary_types_allowed=True)
    source_hash: str
    config_hash: str
    sheet: str
    row_count: int
    column_count: int
    findings: list[Finding] = Field(default_factory=list)
    changes: list[CellChange] = Field(default_factory=list)
    originals: list[dict[str, Any]] = Field(default_factory=list)
    counts_before: dict[str, int] = Field(default_factory=dict)
    filled: dict[str, int] = Field(default_factory=dict)
    maximum_lengths: dict[str, int] = Field(default_factory=dict)


class Cancelled(Exception):
    pass


Progress = Callable[[int, int, str], None]


def display(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, (datetime, date)):
        return value.strftime("%d/%m/%Y")
    if isinstance(value, bool):
        return "Oui" if value else "Non"
    return str(value)


def equal_values(left: Any, right: Any) -> bool:
    if (left is None or left == "") and (right is None or right == ""):
        return True
    if isinstance(left, (datetime, date)) and isinstance(right, (datetime, date)):
        return display(left) == display(right)
    if left is None or right is None:
        return left is right
    if isinstance(left, str) != isinstance(right, str):
        return False
    return left == right
