from __future__ import annotations

import calendar
import re
from datetime import date, datetime
from decimal import Decimal, InvalidOperation
from typing import Any

import phonenumbers

from .dictionary import normalize
from .models import FieldRule


def clean_spaces(value: str) -> str:
    return re.sub(r"\s+", " ", value.replace("\u200b", " ")).strip()


def parse_number(value: Any, rule: FieldRule, integer: bool = False) -> Decimal | int:
    if isinstance(value, bool):
        raise ValueError("Un booléen n'est pas un montant.")
    if isinstance(value, (int, float, Decimal)):
        parsed = Decimal(str(value))
    else:
        text = str(value).strip().replace("\u00a0", " ").replace("\u202f", " ")
        for token in sorted(rule.currency_tokens, key=len, reverse=True):
            text = re.sub(re.escape(token), "", text, flags=re.I).strip()
        if rule.thousands_separator:
            sep = rule.thousands_separator.replace("\u00a0", " ").replace("\u202f", " ")
            escaped = re.escape(sep)
            dec = re.escape(rule.decimal_separator)
            if sep in text:
                if not re.fullmatch(rf"[+-]?\d{{1,3}}(?:{escaped}\d{{3}})+(?:{dec}\d+)?", text):
                    raise ValueError("Groupement des milliers ambigu ou irrégulier.")
                text = text.replace(sep, "")
        if rule.decimal_separator != ".":
            if "." in text:
                raise ValueError("Point inattendu avec le séparateur décimal choisi.")
            text = text.replace(rule.decimal_separator, ".")
        if not re.fullmatch(r"[+-]?\d+(?:\.\d+)?", text):
            raise ValueError("Valeur non numérique avec les séparateurs choisis.")
        try:
            parsed = Decimal(text)
        except InvalidOperation as exc:
            raise ValueError("Nombre non interprétable.") from exc
    if not parsed.is_finite():
        raise ValueError("Nombre non fini.")
    if integer and parsed != parsed.to_integral_value():
        raise ValueError("Un entier est attendu ; aucun arrondi n'a été appliqué.")
    if len(parsed.normalize().as_tuple().digits) > 15:
        raise ValueError("Plus de 15 chiffres significatifs : choisir un stockage texte pour Excel.")
    return int(parsed) if integer else parsed


MONTHS = {
    "janvier": 1,
    "janv": 1,
    "jan": 1,
    "fevrier": 2,
    "fevr": 2,
    "fev": 2,
    "mars": 3,
    "avril": 4,
    "avr": 4,
    "mai": 5,
    "juin": 6,
    "juillet": 7,
    "juil": 7,
    "aout": 8,
    "septembre": 9,
    "sept": 9,
    "octobre": 10,
    "oct": 10,
    "novembre": 11,
    "nov": 11,
    "decembre": 12,
    "dec": 12,
}


def date_options(value: Any, order: str = "dmy") -> tuple[datetime | None, list[datetime], str]:
    if isinstance(value, datetime):
        return value, [], ""
    if isinstance(value, date):
        return datetime.combine(value, datetime.min.time()), [], ""
    if not isinstance(value, str):
        return None, [], "Date non interprétable."
    text = clean_spaces(value)
    try:
        if re.fullmatch(r"\d{4}-\d{2}-\d{2}(?:T.*)?", text):
            return datetime.fromisoformat(text), [], ""
    except ValueError:
        pass
    text = re.sub(r"(?i)(\d{1,2})(?:er|eme|ème|e)\b", r"\1", text)
    folded = normalize(text)
    match = re.fullmatch(r"(\d{1,2}) ([a-z]+) (\d{2,4})", folded)
    if match and match[2] in MONTHS:
        text = f"{match[1]}/{MONTHS[match[2]]}/{match[3]}"
    text = re.sub(r"[.*_\\\-\s]+", "/", text)
    text = re.sub(r"/+", "/", text)
    text = re.sub(r"(?<=\d)[A-Za-z]+$", "", text)
    match = re.fullmatch(r"(\d{1,2})/(\d{1,2})/(\d{2,4})", text)
    if not match:
        compact = re.fullmatch(r"(\d{2})/?(\d{2})/?(\d{4})", text)
        if not compact:
            spans = re.findall(r"\d{1,2}/\d{1,2}/\d{4}", text)
            if len(spans) > 1:
                options = []
                for span in spans:
                    parsed, candidates, _ = date_options(span, order)
                    for candidate in [parsed] if parsed else candidates:
                        if candidate not in options:
                            options.append(candidate)
                return None, options, "Plusieurs dates : choisir une date ou un stockage texte."
            return None, [], "Date incomplète, multiple ou non reconnue."
        match = compact
    first, second, year = map(int, match.groups())
    year = 2000 + year if year < 100 else year
    repair_year = {205: 2025, 225: 2025}.get(year, year)
    options = []
    orders = (
        [(first, second), (second, first)]
        if order == "ask"
        else ([(second, first)] if order == "mdy" else [(first, second)])
    )
    for day, month in orders:
        if 1 <= month <= 12 and 1 <= day <= 31:
            day = min(day, calendar.monthrange(repair_year, month)[1])
            candidate = datetime(repair_year, month, day)
            if candidate not in options:
                options.append(candidate)
    if len(options) == 1:
        candidate = options[0]
        actual_day = second if order == "mdy" else first
        if year == repair_year and candidate.day == actual_day:
            return candidate, [], ""
    return None, options, "Date ambiguë ou correction calendaire à confirmer."


def split_phones(value: Any) -> list[str]:
    if value is None or str(value).strip() == "":
        return []
    text = str(int(value)) if isinstance(value, float) and value.is_integer() else str(value)
    text = re.sub(r"(?i)\b(?:t[eé]l(?:[eé]phone)?|phone)\b\s*[:.]?", "", text)
    text = re.split(r"(?i)(?:\b(?:NIU|NUI|RIB|RCCM|contribuable|compte|fax|email)\b|N[°º]\s*R\.?C)", text)[0]
    text = re.sub(r"(?i)\b(?:et|ou)\b", "/", text)
    chunks = re.split(r"[/;,|\n]+", text)
    numbers = []
    for chunk in chunks:
        stripped = chunk.strip()
        if not stripped:
            continue
        digits = re.sub(r"\D", "", stripped)
        # Only split complete local-number blocks. Never truncate a long unknown number.
        if len(digits) in {18, 27} and all(digits[i] in "62" for i in range(0, len(digits), 9)):
            candidates = [digits[i : i + 9] for i in range(0, len(digits), 9)]
        else:
            candidates = [("+" if stripped.startswith("+") else "") + digits] if digits else [stripped]
        for candidate in candidates:
            if candidate not in numbers:
                numbers.append(candidate)
    return numbers


def valid_phone(number: str, region: str = "CM") -> bool:
    try:
        parsed = phonenumbers.parse(number, region)
        return phonenumbers.is_possible_number(parsed) and phonenumbers.is_valid_number(parsed)
    except phonenumbers.NumberParseException:
        return False


def segment_company(value: Any) -> dict[str, str | None]:
    raw = clean_spaces(str(value or ""))
    stop = r"(?i)(?:\b(?:NIU|NUI|RIB|RCCM|contribuable|compte)\b|N[°º]\s*R\.?C)"
    tel = re.search(r"(?i)\bT[eé]l(?:[eé]phone)?\b\s*[:.]?", raw)
    phone = re.split(stop, raw[tel.end() :])[0] if tel else ""
    head = raw[: tel.start()] if tel else re.split(stop, raw)[0]
    bp_match = re.search(r"(?i)\bB\.?\s*P\.?\s*[:.]?\s*", head)
    bp, location = None, None
    name = head
    if bp_match:
        name = head[: bp_match.start()]
        after = head[bp_match.end() :].strip(" :;,.=-")
        match = re.match(r"(\d[\d ]*)(.*)", after)
        if match:
            bp = re.sub(r"\s+", "", match[1])
            location = match[2].strip(" ;,.-") or None
        else:
            location = after or None
    return {
        "name": name.strip(" ;,.-") or None,
        "bp": bp,
        "phone": "/".join(split_phones(phone)) or None,
        "location": location,
    }
