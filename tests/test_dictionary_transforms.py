from datetime import datetime
from decimal import Decimal

import pytest
from hypothesis import given
from hypothesis import strategies as st

from fdx_atelier.dictionary import parse_dictionary, suggest_mappings
from fdx_atelier.models import DataType, FieldRule, Role
from fdx_atelier.transforms import clean_spaces, date_options, parse_number, segment_company, split_phones


def test_real_tabular_dictionary_types():
    text = "No\tCode_du_champ\tDésignation_du_champ\tType_de_données\tTaille\n"
    text += '1\tNOM\t"Nom du maître"\tAlphanumérique\t200\n'
    text += "2\tTEL\tTéléphone entreprise\tNumérique\t\n"
    text += "3\tDATE\tDate signature\tDATE\tJJ/MM/AAAA\n"
    parsed = parse_dictionary(text)
    assert parsed.fields[0].dtype == DataType.TEXT
    assert parsed.fields[0].max_length == 200
    assert parsed.fields[1].role == Role.PHONE
    assert parsed.fields[1].dtype == DataType.TEXT
    assert parsed.fields[2].dtype == DataType.DATE


def test_markdown_free_text_and_duplicate_headers():
    parsed = parse_dictionary(
        "| Code | Libellé | Type | Taille |\n|---|---|---|---|\n| NOM | Nom | Texte | 50 |"
    )
    assert len(parsed.fields) == 1
    mapping = suggest_mappings(["Nom", "Nom"], parsed.fields)
    assert [m.column for m in mapping] == [1, 2]
    assert not any(m.confirmed for m in mapping)
    assert parse_dictionary("Un numéro ne doit pas être perdu.").notes


@pytest.mark.parametrize(
    "raw,expected",
    [
        ("1 234,50", Decimal("1234.50")),
        ("1\u202f234,567", Decimal("1234.567")),
        ("0", Decimal(0)),
        ("-1 234,50 FCFA", Decimal("-1234.50")),
    ],
)
def test_number_parsing(raw, expected):
    assert parse_number(raw, FieldRule()) == expected


def test_ambiguous_number_and_no_rounding():
    with pytest.raises(ValueError):
        parse_number("12 34,50", FieldRule())
    with pytest.raises(ValueError):
        parse_number("1,5", FieldRule(), integer=True)
    with pytest.raises(ValueError, match="15 chiffres"):
        parse_number("1234567890123456", FieldRule())


def test_date_candidates_are_not_silent_repairs():
    value, options, message = date_options("31/09/2025")
    assert value is None and options == [datetime(2025, 9, 30)]
    assert message
    value, options, _ = date_options("03/04/2025", "ask")
    assert value is None and len(options) == 2
    assert date_options("18 Fév. 2025")[0] == datetime(2025, 2, 18)
    assert date_options("12/03/225")[0] is None
    assert date_options("12/03/225")[1] == [datetime(2025, 3, 12)]
    assert date_options("18/0//2025")[0] is None


def test_phone_no_truncation_and_bank_noise():
    assert split_phones("+2376963254232") == ["+2376963254232"]
    assert split_phones("677712364/699123456") == ["677712364", "699123456"]
    assert split_phones("Tel 677712364 NIU M012345678901A RIB 100030005002") == ["677712364"]


def test_company_segmentation():
    result = segment_company("ETS NABO BP 1 019 Yaoundé Tel 677712364/699123456 NIU M012345678901A")
    assert result == {"name": "ETS NABO", "bp": "1019", "phone": "677712364/699123456", "location": "Yaoundé"}


@given(st.text())
def test_space_normalization_idempotent(text):
    assert clean_spaces(clean_spaces(text)) == clean_spaces(text)
