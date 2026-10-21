from __future__ import annotations

import csv
import re
import unicodedata
from dataclasses import dataclass, field
from pathlib import Path

from rapidfuzz import fuzz

from .models import ColumnMapping, DataType, FieldSpec, Role


def normalize(text: object) -> str:
    value = unicodedata.normalize("NFKD", str(text or ""))
    value = "".join(c for c in value if not unicodedata.combining(c))
    return re.sub(r"[^a-z0-9]+", " ", value.lower()).strip()


def read_text(path: str | Path) -> str:
    raw = Path(path).read_bytes()
    for encoding in ("utf-8-sig", "utf-16", "cp1252"):
        try:
            return raw.decode(encoding)
        except UnicodeError:
            continue
    raise ValueError("Encodage du dictionnaire non reconnu.")


@dataclass
class DictionaryImport:
    fields: list[FieldSpec] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)


ALIASES = {
    "code": {"code", "code du champ", "nom technique", "field code"},
    "label": {"designation du champ", "designation", "libelle", "champ", "nom", "field name"},
    "dtype": {"type de donnees", "type", "datatype", "data type"},
    "size": {"taille", "taille max", "longueur", "longueur max", "size"},
    "required": {"obligatoire", "required", "requis"},
    "pattern": {"motif", "pattern", "regex"},
    "allowed": {"valeurs autorisees", "valeurs", "enum", "allowed values"},
    "minimum": {"minimum", "min"},
    "maximum": {"maximum", "max"},
}


def infer_field(code: str, label: str, dtype: str = "", size: str = "") -> FieldSpec:
    combined = normalize(code + " " + label)
    type_text = normalize(dtype)
    role = Role.STANDARD
    storage = DataType.TEXT
    if "alphanumerique" in type_text or any(word in type_text for word in ("texte", "string", "text")):
        storage = DataType.TEXT
    elif "date" in type_text or combined.startswith("date "):
        storage = DataType.DATE
    elif any(word in type_text for word in ("numerique", "decimal", "number", "float")):
        storage = DataType.DECIMAL
    elif any(word in type_text for word in ("entier", "integer", "int")):
        storage = DataType.INTEGER
    elif "bool" in type_text:
        storage = DataType.BOOLEAN
    if any(word in combined.split() for word in ("tel", "telephone", "phone")):
        role, storage = Role.PHONE, DataType.TEXT
    elif any(word in combined.split() for word in ("niu", "nui", "identifiant", "rccm", "rib")):
        role, storage = Role.IDENTIFIER, DataType.TEXT
    elif ("nom" in combined or "name" in combined) and ("entreprise" in combined or "en sel" in combined):
        role = Role.COMPANY
    length = int(size) if re.fullmatch(r"\d+", str(size).strip()) and int(size) > 0 else None
    return FieldSpec(code=code.strip(), label=label.strip(' "'), dtype=storage, role=role, max_length=length)


def parse_dictionary(text: str) -> DictionaryImport:
    result = DictionaryImport()
    lines = text.splitlines()
    headers: dict[str, int] | None = None
    delimiter = "\t"
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if re.fullmatch(r"[\s|:\-]+", stripped):
            continue
        if headers is None:
            candidates = ["\t", "|", ";"]
            for sep in candidates:
                cells = next(csv.reader([stripped.strip("|")], delimiter=sep))
                found = {}
                for i, cell in enumerate(cells):
                    normalized = normalize(cell)
                    for key, aliases in ALIASES.items():
                        if normalized in aliases:
                            found[key] = i
                if "label" in found and ("code" in found or "dtype" in found):
                    headers, delimiter = found, sep
                    break
            if headers is None:
                match = re.match(r"^[-*]?\s*([A-Za-z][\w ]*)\s*[:|]\s*(.+)$", stripped)
                if match and re.search(r"(?i)date|num[eé]rique|texte|alphanum|entier", match[2]):
                    details = match[2]
                    size_match = re.search(r"\b(\d+)\s*(?:car|max)", details, re.I)
                    result.fields.append(
                        infer_field(
                            match[1].strip(), match[1].strip(), details, size_match[1] if size_match else ""
                        )
                    )
                else:
                    result.notes.append(stripped)
            continue
        cells = next(csv.reader([stripped.strip("|")], delimiter=delimiter))

        def value(key: str) -> str:
            idx = headers.get(key)
            return cells[idx].strip().strip('"') if idx is not None and idx < len(cells) else ""

        label = value("label")
        if not label:
            result.notes.append(stripped)
            continue
        code = value("code") or re.sub(r"\W+", "_", normalize(label)).upper()
        spec = infer_field(code, label, value("dtype"), value("size"))
        spec.required = normalize(value("required")) in {"oui", "yes", "true", "1"}
        spec.pattern = value("pattern")
        spec.allowed_values = [x.strip() for x in value("allowed").split(",") if x.strip()]
        for attr in ("minimum", "maximum"):
            if value(attr):
                from decimal import Decimal, InvalidOperation

                try:
                    setattr(spec, attr, Decimal(value(attr).replace(",", ".")))
                except InvalidOperation:
                    result.notes.append(f"Borne à confirmer : {code}, {value(attr)}")
        result.fields.append(spec)
    if not result.fields:
        result.notes.insert(0, "Aucun tableau reconnu : ajoutez les champs dans le formulaire.")
    return result


def suggest_mappings(headers: list[object], fields: list[FieldSpec]) -> list[ColumnMapping]:
    def words(value):
        text = normalize(value)
        text = re.sub(r"\bmo\b", "maitre ouvrage", text)
        text = re.sub(r"\bmod\b", "maitre ouvrage delegue", text)
        return " ".join(
            w for w in text.split() if w not in {"de", "du", "des", "le", "la", "l", "d", "en", "et"}
        )

    pairs = []
    for position, header in enumerate(headers, 1):
        query = words(header)
        if not query:
            continue
        inferred = infer_field("", str(header))
        for spec in fields:
            label = words(spec.label)
            if normalize(header) in {normalize(spec.label), normalize(spec.code)} or query == label:
                score = 100
            else:
                score = fuzz.WRatio(query, label)
                if inferred.role in {Role.PHONE, Role.IDENTIFIER}:
                    score = min(99, score + 20) if inferred.role == spec.role else score * 0.5
                if label and query.split()[0] == label.split()[0]:
                    score = min(99, score + 8)
            pairs.append((score, position, spec.code))
    proposals = [ColumnMapping(column=i) for i in range(1, len(headers) + 1)]
    taken_columns, taken_fields = set(), set()
    # Reserve strongest matches first so a broad label cannot steal an exact match.
    for score, position, code in sorted(pairs, key=lambda p: (-p[0], p[1], p[2])):
        if score < 55 or position in taken_columns or code in taken_fields:
            continue
        proposals[position - 1] = ColumnMapping(column=position, field_code=code, score=round(score, 1))
        taken_columns.add(position)
        taken_fields.add(code)
    return proposals
