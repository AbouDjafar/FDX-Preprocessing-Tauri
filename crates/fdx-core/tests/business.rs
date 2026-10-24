use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use fdx_core::{
    dictionary::{infer_field, normalize, parse_dictionary, suggest_mappings},
    engine::{analyze, config_hash, converted, decisions_hash, validate_cell, validate_config},
    models::*,
    transforms::{
        clean_spaces, date_options, decimal, iso_date, parse_number, segment_company, split_phones,
        split_phones_region, valid_phone,
    },
    workbooks::sha256,
};
use proptest::prelude::*;

fn field(code: &str, dtype: &str, role: &str) -> FieldSpec {
    FieldSpec {
        code: code.into(),
        label: code.into(),
        dtype: dtype.into(),
        role: role.into(),
        ..Default::default()
    }
}

fn fixture() -> (ProjectConfig, WorkbookData) {
    let fields = vec![
        field("NOM", "text", "company"),
        field("TEL", "text", "phone"),
        field("MON", "decimal", "standard"),
        field("DATE", "date", "standard"),
        field("NIU", "text", "identifier"),
        field("SIGN", "date", "standard"),
    ];
    let mappings = [1, 2, 3, 4, 5, 7]
        .into_iter()
        .zip(&fields)
        .map(|(column, spec)| ColumnMapping {
            column,
            field_code: Some(spec.code.clone()),
            confirmed: true,
            score: 100.0,
        })
        .collect();
    let config = ProjectConfig {
        sheet: "Donn\u{e9}es!2025".into(),
        fields,
        mappings,
        ..Default::default()
    };
    let mut sheet = SheetData {
        name: config.sheet.clone(),
        rows: 3,
        columns: 7,
        ..Default::default()
    };
    for (row, values) in [
        (
            2,
            vec![
                CellValue::text("  Entreprise A  "),
                CellValue::text("+237699123456/677123456"),
                CellValue::text("1 234,50"),
                CellValue::text("31/09/2025"),
                CellValue::text("00123"),
                CellValue::text("inchange"),
                CellValue::typed("date", "2025-08-01"),
            ],
        ),
        (
            3,
            vec![
                CellValue::text("Entreprise B"),
                CellValue::text("+390498976845"),
                CellValue::typed("decimal", "1250.12345678901"),
                CellValue::text("18 F\u{e9}v. 2025"),
                CellValue::text("M123456789012X"),
                CellValue::typed("formula", "=SUM(C2:C3)"),
                CellValue::typed("date", "2025-04-01"),
            ],
        ),
    ] {
        for (col, value) in values.into_iter().enumerate() {
            sheet.cells.insert((row, col as u32 + 1), value);
        }
    }
    (
        config,
        WorkbookData {
            source_hash: "a".repeat(64),
            sheets: vec![sheet],
        },
    )
}

fn run(config: &ProjectConfig, source: &WorkbookData, decisions: &Decisions) -> AnalysisResult {
    analyze(config, source, decisions, &JobControl::default()).unwrap()
}

fn decision(action: &str, value: CellValue) -> Decision {
    Decision {
        action: action.into(),
        value,
        ..Default::default()
    }
}

fn value_after(result: &AnalysisResult, source: &WorkbookData, row: u32, col: u32) -> CellValue {
    result
        .changes
        .iter()
        .find(|c| c.row == row && c.column == col)
        .map(|c| c.after.clone())
        .unwrap_or_else(|| source.sheets[0].cell(row, col))
}

#[test]
fn dictionary_reads_french_tsv_including_trailing_empty_columns() {
    let parsed = parse_dictionary("\u{feff}No\tCode_du_champ\tD\u{e9}signation_du_champ\tType_de_donn\u{e9}es\tTaille\n1\tNOM\t\"Nom du ma\u{ee}tre\"\tAlphanum\u{e9}rique\t200\n2\tTEL\tT\u{e9}l\u{e9}phone entreprise\tNum\u{e9}rique\t\n3\tDATE\tDate signature\tDATE\tJJ/MM/AAAA\n");
    assert_eq!(parsed.fields.len(), 3);
    assert_eq!(parsed.fields[0].dtype, "text");
    assert_eq!(parsed.fields[0].max_length, Some(200));
    assert_eq!(parsed.fields[1].role, "phone");
    assert_eq!(parsed.fields[1].dtype, "text");
    assert_eq!(parsed.fields[2].date_format, "DD/MM/YYYY");
}

#[test]
fn dictionary_parses_quoted_delimiters_and_constraints_without_float() {
    let parsed = parse_dictionary("Code;Libelle;Type;Obligatoire;Motif;Valeurs autorisees;Minimum;Maximum;Role\nM;\"Montant; net\";Decimal;Oui;;\"1,2\";0,1;12345678901234567890,123456789;standard\n");
    assert_eq!(parsed.fields.len(), 1);
    let field = &parsed.fields[0];
    assert_eq!(field.label, "Montant; net");
    assert!(field.required);
    assert_eq!(field.allowed_values, ["1", "2"]);
    assert_eq!(field.minimum.as_deref(), Some("0.1"));
    assert_eq!(
        field.maximum.as_deref(),
        Some("12345678901234567890.123456789")
    );
    let multiline = parse_dictionary("Code;Libelle;Type\nN;\"Nom\ndu maitre\";Texte\n");
    assert_eq!(multiline.fields[0].label, "Nom\ndu maitre");
}

#[test]
fn dictionary_preserves_regex_escapes_duplicates_and_unrecognized_notes() {
    let parsed = parse_dictionary("| Code | Libelle | Type | Motif |\n|---|---|---|---|\n| N | Nom | Texte | \\d{2}(?:A\\|B) |\n| N | Autre | Texte | [ |\nTexte libre non executable\n");
    assert_eq!(parsed.fields.len(), 2);
    assert_eq!(parsed.fields[0].pattern, r"\d{2}(?:A|B)");
    assert!(parsed.notes.iter().any(|n| n.contains("duplique")));
    assert!(parsed.notes.iter().any(|n| n.contains("Motif invalide")));
    assert!(parsed.notes.iter().any(|n| n.contains("Texte libre")));
    assert!(
        !parse_dictionary("Aucune regle arbitraire ne doit etre executee.")
            .notes
            .is_empty()
    );
    let bad = parse_dictionary("Code;Libelle;Type;Minimum\nM;Montant;Decimal;inconnu");
    assert_eq!(bad.fields[0].minimum.as_deref(), Some("inconnu"));
    assert!(!bad.notes.is_empty());
}

#[test]
fn dictionary_inference_is_text_safe_and_free_definitions_need_confirmation() {
    assert_eq!(
        infer_field("NIU", "Numero", "Numerique", "14").dtype,
        "text"
    );
    assert_eq!(infer_field("TEL", "Telephone", "entier", "").role, "phone");
    assert_eq!(
        infer_field("Q", "Quantite", "entier positif", "").dtype,
        "integer"
    );
    let parsed =
        parse_dictionary("- NOM: texte 50 caracteres max\nUne annee ne doit jamais etre inventee.");
    assert_eq!(parsed.fields[0].max_length, Some(50));
    assert_eq!(parsed.notes.len(), 2);
    assert_eq!(
        normalize("Ma\u{ee}tre d'\u{153}uvre \u{c9}TAT"),
        "maitre d oeuvre etat"
    );
}

#[test]
fn mappings_keep_excel_positions_reserve_exact_matches_and_never_confirm() {
    let fields = vec![
        FieldSpec {
            label: "T\u{e9}l\u{e9}phone de l'entreprise".into(),
            ..field("TEL", "text", "phone")
        },
        field("NIU", "text", "identifier"),
        FieldSpec {
            label: "Nom de l'entreprise".into(),
            ..field("NOM", "text", "company")
        },
    ];
    let headers = [
        "Categorie",
        "Nom de l'entreprise",
        "NIU",
        "Telephone de l'entreprise",
        "Nom de l'entreprise",
    ]
    .map(CellValue::text);
    let mappings = suggest_mappings(&headers, &fields);
    assert_eq!(
        mappings
            .iter()
            .map(|m| m.field_code.as_deref())
            .collect::<Vec<_>>(),
        [None, Some("NOM"), Some("NIU"), Some("TEL"), None]
    );
    assert_eq!(
        mappings.iter().map(|m| m.column).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    assert!(mappings.iter().all(|m| !m.confirmed));
    let mo = FieldSpec {
        label: "Nom du maitre d'ouvrage delegue".into(),
        ..field("MOD", "text", "standard")
    };
    assert_eq!(
        suggest_mappings(&[CellValue::text("Nom MOD")], &[mo])[0].score,
        100.0
    );
}

#[test]
fn exact_decimal_parsing_never_uses_binary_float_or_rounds() {
    for (raw, expected) in [
        ("1 234,50", "1234.50"),
        ("1\u{202f}234,567", "1234.567"),
        ("1\u{a0}234,50", "1234.50"),
        ("0", "0"),
        ("-1 234,50 FCFA", "-1234.50"),
        ("xaf 123,50", "123.50"),
        (
            "0,000000000000000000123456789012345",
            "0.000000000000000000123456789012345",
        ),
    ] {
        assert_eq!(
            parse_number(&CellValue::text(raw), &FieldRule::default(), false).unwrap(),
            decimal(expected).unwrap()
        );
    }
    for raw in [
        "12 34,50",
        "1,5,6",
        "1.234,50",
        "1e3",
        "NaN",
        "inf",
        "12FCFA34",
        "1234567890123456",
    ] {
        assert!(
            parse_number(&CellValue::text(raw), &FieldRule::default(), false).is_err(),
            "{raw}"
        );
    }
    assert!(parse_number(&CellValue::text("1,5"), &FieldRule::default(), true).is_err());
    assert!(parse_number(
        &CellValue::typed("boolean", "true"),
        &FieldRule::default(),
        false
    )
    .is_err());
    assert_eq!(
        parse_number(
            &CellValue::typed("decimal", "1.25E-20"),
            &FieldRule::default(),
            false
        )
        .unwrap(),
        decimal("0.0000000000000000000125").unwrap()
    );
    let english = FieldRule {
        decimal_separator: ".".into(),
        thousands_separator: ",".into(),
        ..Default::default()
    };
    assert_eq!(
        parse_number(&CellValue::text("1,234.50"), &english, false).unwrap(),
        decimal("1234.50").unwrap()
    );
    assert!(decimal("1e999999999999999999999999999999").is_err());
    assert!(decimal("1e-10001").is_err());
}

#[test]
fn french_dates_explicit_order_and_calendar_proposals() {
    for (raw, expected) in [
        ("18 F\u{e9}v. 2025", "2025-02-18"),
        ("1er ao\u{fb}t 2025", "2025-08-01"),
        ("18022025", "2025-02-18"),
        ("18/022025", "2025-02-18"),
        ("18/02/25", "2025-02-18"),
        ("2025-02-18T00:00:00", "2025-02-18"),
    ] {
        assert_eq!(
            date_options(&CellValue::text(raw), "dmy").parsed,
            iso_date(expected),
            "{raw}"
        );
    }
    assert_eq!(
        date_options(&CellValue::text("03/04/2025"), "mdy").parsed,
        iso_date("2025-03-04")
    );
    let ambiguous = date_options(&CellValue::text("03/04/2025"), "ask");
    assert!(ambiguous.parsed.is_none());
    assert_eq!(ambiguous.choices.len(), 2);
    assert_eq!(
        date_options(&CellValue::text("04/18/2025"), "ask").parsed,
        iso_date("2025-04-18")
    );
    for raw in ["31/09/2025", "2025-09-31"] {
        let invalid = date_options(&CellValue::text(raw), "dmy");
        assert!(invalid.parsed.is_none());
        assert_eq!(invalid.choices, [iso_date("2025-09-30").unwrap()]);
    }
    let repaired = date_options(&CellValue::text("12/03/225"), "dmy");
    assert!(repaired.parsed.is_none());
    assert_eq!(repaired.choices, [iso_date("2025-03-12").unwrap()]);
    for raw in ["18/0//2025", "18/02", "00/02/2025", "2025-02-18garbage"] {
        assert!(date_options(&CellValue::text(raw), "dmy").parsed.is_none());
    }
    assert!(date_options(&CellValue::typed("integer", "45678"), "dmy")
        .parsed
        .is_none());
    let multiple = date_options(&CellValue::text("18/02/2025 et 20/03/2025"), "dmy");
    assert!(multiple.parsed.is_none());
    assert_eq!(multiple.choices.len(), 2);
}

#[test]
fn phones_preserve_international_prefixes_and_reject_noise_without_truncation() {
    assert_eq!(split_phones("+2376963254232"), ["+2376963254232"]);
    assert_eq!(
        split_phones("Tel 677712364 NIU M012345678901A RIB 100030005002"),
        ["677712364"]
    );
    assert_eq!(
        split_phones("677712364/699123456 et 677712364"),
        ["677712364", "699123456"]
    );
    assert_eq!(
        split_phones("677712364699123456"),
        ["677712364", "699123456"]
    );
    assert_eq!(split_phones("+677712364699123456"), ["+677712364699123456"]);
    assert_eq!(
        split_phones_region("677712364699123456", "FR"),
        ["677712364699123456"]
    );
    assert!(valid_phone("+390498976845", "CM"));
    assert!(valid_phone("+237699123456", "CM"));
    assert!(!valid_phone("+2376963254232", "CM"));
    assert!(!valid_phone("699123456poste10", "CM"));
    assert_eq!(split_phones("699123456 poste 10"), ["699123456 poste 10"]);
}

#[test]
fn segmentation_extracts_only_known_markers() {
    let parts = segment_company(
        "ETS NABO BP 1 019 Yaound\u{e9} Tel 677712364/699123456 NIU M012345678901A",
    );
    assert_eq!(parts["name"].as_deref(), Some("ETS NABO"));
    assert_eq!(parts["bp"].as_deref(), Some("1019"));
    assert_eq!(parts["phone"].as_deref(), Some("677712364/699123456"));
    assert_eq!(parts["location"].as_deref(), Some("Yaound\u{e9}"));
    let plain = segment_company("Entreprise sans marqueur");
    assert_eq!(plain["name"].as_deref(), Some("Entreprise sans marqueur"));
    assert!(plain["location"].is_none());
    assert_eq!(
        segment_company("BPlus SARL")["name"].as_deref(),
        Some("BPlus SARL")
    );
}

#[test]
fn full_analysis_preserves_source_and_coordinates_and_records_originals() {
    let (config, source) = fixture();
    let before = source.sheets[0].cells.clone();
    let result = run(&config, &source, &Decisions::new());
    assert_eq!(result.row_count, 2);
    assert_eq!(result.column_count, 7);
    assert_eq!(source.sheets[0].cells, before);
    assert!(result
        .findings
        .iter()
        .any(|f| f.kind == "phone_multiple" && f.key == cell_key(&config.sheet, 2, 2)));
    assert!(result
        .findings
        .iter()
        .any(|f| f.kind == "date_invalid" && f.row == 2 && f.column == 4));
    assert_eq!(
        value_after(&result, &source, 2, 3),
        CellValue::typed("decimal", "1234.50")
    );
    assert_eq!(
        value_after(&result, &source, 3, 4),
        CellValue::typed("date", "2025-02-18")
    );
    assert_eq!(
        value_after(&result, &source, 2, 5),
        CellValue::text("00123")
    );
    assert_eq!(
        value_after(&result, &source, 3, 3),
        source.sheets[0].cell(3, 3)
    );
    assert!(!result.changes.iter().any(|c| c.column == 6));
    for change in &result.changes {
        assert_eq!(
            change.before,
            source.sheets[0].cell(change.row, change.column)
        );
        assert!(result
            .originals
            .iter()
            .any(|o| o.key == change.key && o.before == change.before));
    }
    assert_eq!(result.source_hash, source.source_hash);
    assert_eq!(result.config_hash, config_hash(&config).unwrap());
    assert_eq!(
        result.decisions_hash,
        decisions_hash(&Decisions::new()).unwrap()
    );
}

#[test]
fn decisions_override_every_destructive_policy_and_keep_is_not_validation() {
    let (mut config, source) = fixture();
    config.rules.insert(
        "TEL".into(),
        FieldRule {
            phone_policy: "first".into(),
            ..Default::default()
        },
    );
    let key = cell_key(&config.sheet, 2, 2);
    assert_eq!(
        value_after(&run(&config, &source, &Decisions::new()), &source, 2, 2),
        CellValue::text("+237699123456")
    );
    let mut decisions = Decisions::from([(key.clone(), Decision::default())]);
    let kept = run(&config, &source, &decisions);
    assert!(!kept.changes.iter().any(|c| c.key == key));
    assert!(kept
        .findings
        .iter()
        .any(|f| f.key == key && f.kind == "phone_multiple"));
    decisions.insert(
        key.clone(),
        Decision {
            allow_multiple: true,
            ..decision("set", CellValue::text("+237699123456/677123456"))
        },
    );
    assert!(!run(&config, &source, &decisions)
        .findings
        .iter()
        .any(|f| f.key == key && f.kind == "phone_multiple"));
    config.fields[0].max_length = Some(3);
    config.fields[0].pattern = "[A-Z]+".into();
    config.rules.insert(
        "NOM".into(),
        FieldRule {
            length_policy: "truncate".into(),
            invalid_policy: "clear".into(),
            replacements: BTreeMap::from([("  lower case  ".into(), "OTHER".into())]),
            ..Default::default()
        },
    );
    decisions.insert(
        cell_key(&config.sheet, 2, 1),
        decision("set", CellValue::text("  lower case  ")),
    );
    let result = run(&config, &source, &decisions);
    assert_eq!(
        value_after(&result, &source, 2, 1),
        CellValue::text("  lower case  ")
    );
    assert!(result
        .findings
        .iter()
        .any(|f| f.row == 2 && f.column == 1 && f.kind == "length"));
    assert!(result
        .findings
        .iter()
        .any(|f| f.row == 2 && f.column == 1 && f.kind == "pattern"));
}

#[test]
fn impossible_dates_need_an_exact_decision_and_chronology_never_changes_years() {
    let (mut config, source) = fixture();
    config.chronology = vec![ChronologyRule {
        before: "DATE".into(),
        after: "SIGN".into(),
        enabled: true,
    }];
    let decisions = Decisions::from([(
        cell_key(&config.sheet, 2, 4),
        decision("set", CellValue::text("2025-09-30")),
    )]);
    let result = run(&config, &source, &decisions);
    assert!(!result
        .findings
        .iter()
        .any(|f| f.row == 2 && f.kind == "date_invalid"));
    let issue = result
        .findings
        .iter()
        .find(|f| f.kind == "chronology")
        .unwrap();
    assert_eq!((issue.row, issue.column), (2, 4));
    assert_eq!(issue.current, CellValue::typed("date", "2025-09-30"));
    assert_eq!(issue.original, CellValue::text("31/09/2025"));
    config.chronology[0].enabled = false;
    assert!(!run(&config, &source, &decisions)
        .findings
        .iter()
        .any(|f| f.kind == "chronology"));
    let ambiguous = Decisions::from([(
        cell_key(&config.sheet, 2, 4),
        decision("set", CellValue::text("31/09/2025")),
    )]);
    assert!(run(&config, &source, &ambiguous)
        .findings
        .iter()
        .any(|f| f.row == 2 && f.kind == "date_invalid"));
}

#[test]
fn constraints_accumulate_and_compare_arbitrary_precision_bounds_exactly() {
    let (mut config, mut source) = fixture();
    config.fields[0].max_length = Some(3);
    config.fields[0].pattern = "[A-Z]+".into();
    config.fields[0].allowed_values = vec!["OK".into()];
    config.fields[2].minimum = Some("1234.500000000000000000000000000001".into());
    config.fields[2].maximum = Some("1250.123456789009999999999999999999".into());
    source.sheets[0]
        .cells
        .insert((3, 3), CellValue::typed("decimal", "1250.12345678901"));
    let result = run(&config, &source, &Decisions::new());
    let kinds: BTreeSet<_> = result
        .findings
        .iter()
        .filter(|f| f.row == 2 && f.column == 1)
        .map(|f| f.kind.as_str())
        .collect();
    assert!(kinds.contains("length") && kinds.contains("pattern") && kinds.contains("constraint"));
    assert!(result
        .findings
        .iter()
        .any(|f| f.row == 2 && f.column == 3 && f.message.ends_with("minimum")));
    assert!(result
        .findings
        .iter()
        .any(|f| f.row == 3 && f.column == 3 && f.message.ends_with("maximum")));
    assert_eq!(result.counts_before["NOM"], 2);
}

#[test]
fn pattern_supports_lookahead_full_matching_and_explicit_clear() {
    let spec = FieldSpec {
        pattern: r"(?=.*[A-Z])(?=.*\d)[A-Z0-9]+".into(),
        required: true,
        ..field("ID", "text", "identifier")
    };
    for (raw, expected) in [("AB12", true), ("abcd", false), ("AB12\n", false)] {
        let value = CellValue::text(raw);
        let issues = validate_cell(
            "S",
            2,
            1,
            &value,
            &value,
            &spec,
            &FieldRule::default(),
            false,
        )
        .unwrap();
        assert_eq!(issues.is_empty(), expected);
    }
    let rule = FieldRule {
        invalid_policy: "clear".into(),
        ..Default::default()
    };
    let raw = CellValue::text("invalid");
    let (cleared, _) = converted(&raw, &spec, &rule).unwrap();
    assert!(cleared.is_empty());
    let issues = validate_cell("S", 2, 1, &raw, &cleared, &spec, &rule, false).unwrap();
    assert_eq!(issues[0].kind, "required");
}

#[test]
fn unicode_lengths_and_exact_replacements_are_not_byte_operations() {
    let spec = FieldSpec {
        max_length: Some(2),
        ..field("N", "text", "standard")
    };
    let value = CellValue::text("\u{e9}\u{e8}\u{e0}");
    assert_eq!(
        converted(&value, &spec, &FieldRule::default()).unwrap().0,
        value
    );
    let rule = FieldRule {
        length_policy: "truncate".into(),
        ..Default::default()
    };
    assert_eq!(
        converted(&value, &spec, &rule).unwrap().0,
        CellValue::text("\u{e9}\u{e8}")
    );
    let rule = FieldRule {
        normalize_spaces: false,
        replacements: BTreeMap::from([("Exact".into(), "Oui".into())]),
        ..Default::default()
    };
    let spec = field("N", "text", "standard");
    assert_eq!(
        converted(&CellValue::text("Exact"), &spec, &rule)
            .unwrap()
            .0,
        CellValue::text("Oui")
    );
    assert_eq!(
        converted(&CellValue::text("exact"), &spec, &rule)
            .unwrap()
            .0,
        CellValue::text("exact")
    );
}

#[test]
fn segmentation_conflicts_keep_raw_destinations_and_individual_choices_win() {
    let (mut config, mut source) = fixture();
    source.sheets[0].cells.insert(
        (2, 1),
        CellValue::text("ETS ALPHA BP: 123 Douala Tel: 699123456"),
    );
    config.segmentation = SegmentationRule {
        enabled: true,
        source_column: 1,
        name_column: Some(1),
        bp_column: Some(5),
        phone_column: Some(2),
        location_column: Some(6),
        overwrite: false,
    };
    let result = run(&config, &source, &Decisions::new());
    let issue = result
        .findings
        .iter()
        .find(|f| f.row == 2 && f.column == 5 && f.kind == "segmentation_conflict")
        .unwrap();
    assert_eq!(issue.proposed, Some(CellValue::text("123")));
    assert_eq!(
        value_after(&result, &source, 2, 5),
        CellValue::text("00123")
    );
    assert_eq!(
        value_after(&result, &source, 2, 1),
        CellValue::text("ETS ALPHA")
    );
    let decisions = Decisions::from([(
        cell_key(&config.sheet, 2, 5),
        decision("set", CellValue::text("123")),
    )]);
    let fixed = run(&config, &source, &decisions);
    assert_eq!(value_after(&fixed, &source, 2, 5), CellValue::text("123"));
    assert!(!fixed
        .findings
        .iter()
        .any(|f| f.row == 2 && f.column == 5 && f.kind == "segmentation_conflict"));
    assert!(fixed
        .originals
        .iter()
        .any(|c| c.row == 2 && c.column == 1 && c.before == source.sheets[0].cell(2, 1)));
    config.segmentation.overwrite = true;
    let decisions = Decisions::from([(cell_key(&config.sheet, 2, 5), Decision::default())]);
    assert_eq!(
        value_after(&run(&config, &source, &decisions), &source, 2, 5),
        CellValue::text("00123")
    );
}

#[test]
fn segmentation_validates_generated_values_and_protects_formula_destinations() {
    let (mut config, mut source) = fixture();
    config.segmentation = SegmentationRule {
        enabled: true,
        source_column: 1,
        name_column: Some(1),
        phone_column: Some(2),
        location_column: Some(6),
        overwrite: true,
        ..Default::default()
    };
    source.sheets[0]
        .cells
        .insert((2, 1), CellValue::text("ETS X BP 12 Douala Tel 123"));
    source.sheets[0]
        .cells
        .insert((2, 6), CellValue::typed("formula", "=1+1"));
    let decisions = Decisions::from([(
        cell_key(&config.sheet, 2, 6),
        decision("clear", CellValue::empty()),
    )]);
    let result = run(&config, &source, &decisions);
    assert!(result
        .findings
        .iter()
        .any(|f| f.row == 2 && f.column == 2 && f.kind == "phone_invalid"));
    assert!(result
        .findings
        .iter()
        .any(|f| f.row == 2 && f.column == 6 && f.kind == "formula"));
    assert!(!result.changes.iter().any(|c| c.row == 2 && c.column == 6));
}

#[test]
fn source_formulas_survive_all_policies_and_formula_like_text_stays_text() {
    let (mut config, mut source) = fixture();
    source.sheets[0]
        .cells
        .insert((2, 1), CellValue::typed("formula", "=SUM(A3:A5)"));
    config.fields[0].max_length = Some(1);
    config.fields[0].pattern = "NEVER".into();
    config.rules.insert(
        "NOM".into(),
        FieldRule {
            length_policy: "clear".into(),
            invalid_policy: "clear".into(),
            ..Default::default()
        },
    );
    for action in ["clear", "set", "keep"] {
        let choices = Decisions::from([(
            cell_key(&config.sheet, 2, 1),
            decision(action, CellValue::text("replacement")),
        )]);
        let result = run(&config, &source, &choices);
        assert!(!result.changes.iter().any(|c| c.row == 2 && c.column == 1));
        assert!(result
            .findings
            .iter()
            .any(|f| f.row == 2 && f.column == 1 && f.kind == "formula"));
    }
    let choices = Decisions::from([(
        cell_key(&config.sheet, 3, 1),
        decision("set", CellValue::text("=1+1")),
    )]);
    assert_eq!(
        value_after(&run(&config, &source, &choices), &source, 3, 1),
        CellValue::text("=1+1")
    );
}

#[test]
fn missing_cells_and_empty_rows_are_validated_to_the_last_source_row() {
    let (mut config, mut source) = fixture();
    source.sheets[0].rows = 503;
    source.sheets[0]
        .cells
        .insert((503, 3), CellValue::text("2 000,50"));
    config.fields[0].required = true;
    let result = run(&config, &source, &Decisions::new());
    assert_eq!(result.row_count, 502);
    assert_eq!(
        result
            .findings
            .iter()
            .filter(|f| f.field_code == "NOM" && f.kind == "required")
            .count(),
        500
    );
    assert_eq!(
        value_after(&result, &source, 503, 3),
        CellValue::typed("decimal", "2000.50")
    );
    assert_eq!(result.filled["NOM"], 2);
    assert!(result
        .findings
        .iter()
        .any(|f| f.row == 502 && f.column == 1));
}

#[test]
fn conversion_can_be_disabled_without_hiding_invalid_storage() {
    let (mut config, source) = fixture();
    config.rules.insert(
        "MON".into(),
        FieldRule {
            convert: false,
            ..Default::default()
        },
    );
    config.rules.insert(
        "DATE".into(),
        FieldRule {
            convert: false,
            ..Default::default()
        },
    );
    let result = run(&config, &source, &Decisions::new());
    assert_eq!(
        value_after(&result, &source, 2, 3),
        CellValue::text("1 234,50")
    );
    assert!(result.findings.iter().any(|f| f.row == 2
        && f.column == 3
        && f.kind == "number_invalid"
        && f.proposed.is_some()));
    assert!(result
        .findings
        .iter()
        .any(|f| f.row == 3 && f.column == 4 && f.kind == "date_invalid"));
}

#[test]
fn phones_all_policies_custom_separator_and_newlines() {
    let spec = field("TEL", "text", "phone");
    let raw = CellValue::text("123/+237699123456\n+390498976845");
    for (policy, expected) in [
        ("first", CellValue::text("123")),
        ("first_valid", CellValue::text("+237699123456")),
        ("clear", CellValue::empty()),
        (
            "all",
            CellValue::text("123 :: +237699123456 :: +390498976845"),
        ),
    ] {
        let rule = FieldRule {
            phone_policy: policy.into(),
            phone_separator: " :: ".into(),
            ..Default::default()
        };
        let (value, _) = converted(&raw, &spec, &rule).unwrap();
        assert_eq!(value, expected);
        let issues = validate_cell("S", 2, 1, &raw, &value, &spec, &rule, false).unwrap();
        if policy == "all" {
            assert!(!issues.iter().any(|f| f.kind == "phone_multiple"));
            assert!(issues.iter().any(|f| f.kind == "phone_invalid"));
        }
    }
    let rule = FieldRule {
        phone_policy: "first_valid".into(),
        ..Default::default()
    };
    assert_eq!(
        converted(&CellValue::text("123/456"), &spec, &rule)
            .unwrap()
            .0,
        CellValue::text("123/456")
    );
}

#[test]
fn boolean_and_identifier_conversion_preserve_storage_semantics() {
    let boolean = field("B", "boolean", "standard");
    for (raw, expected) in [
        ("OUI", "true"),
        ("non", "false"),
        ("1", "true"),
        ("0", "false"),
    ] {
        assert_eq!(
            converted(&CellValue::text(raw), &boolean, &FieldRule::default())
                .unwrap()
                .0,
            CellValue::typed("boolean", expected)
        );
    }
    let identifier = field("NIU", "text", "identifier");
    assert_eq!(
        converted(
            &CellValue::text("000123"),
            &identifier,
            &FieldRule::default()
        )
        .unwrap()
        .0,
        CellValue::text("000123")
    );
    assert_eq!(
        converted(
            &CellValue::typed("decimal", "123.0"),
            &identifier,
            &FieldRule::default()
        )
        .unwrap()
        .0,
        CellValue::text("123")
    );
    assert!(CellValue::typed("decimal", "1.0").equivalent(&CellValue::typed("integer", "1")));
    assert!(!CellValue::text("1").equivalent(&CellValue::typed("integer", "1")));
    assert!(!CellValue::typed("boolean", "true").equivalent(&CellValue::typed("integer", "1")));
}

#[test]
fn configuration_rejects_inconsistent_contracts_before_processing() {
    let (config, _) = fixture();
    let check = |mutator: fn(&mut ProjectConfig)| {
        let mut bad = config.clone();
        mutator(&mut bad);
        assert!(validate_config(&bad).is_err());
    };
    check(|c| c.mappings[0].confirmed = false);
    check(|c| c.mappings[1].field_code = Some("NOM".into()));
    check(|c| c.mappings[1].column = 1);
    check(|c| c.fields[1].code = "NOM".into());
    check(|c| c.fields[0].label.clear());
    check(|c| c.fields[0].max_length = Some(0));
    check(|c| c.fields[0].pattern = "[".into());
    check(|c| c.fields[2].minimum = Some("NaN".into()));
    check(|c| {
        c.fields[2].minimum = Some("2".into());
        c.fields[2].maximum = Some("1".into());
    });
    check(|c| {
        c.rules.insert(
            "MON".into(),
            FieldRule {
                decimal_separator: ",".into(),
                thousands_separator: ",".into(),
                ..Default::default()
            },
        );
    });
    check(|c| {
        c.rules.insert(
            "TEL".into(),
            FieldRule {
                phone_region: "ZZ".into(),
                ..Default::default()
            },
        );
    });
    check(|c| {
        c.rules.insert(
            "DATE".into(),
            FieldRule {
                date_order: "guess".into(),
                ..Default::default()
            },
        );
    });
    check(|c| {
        c.chronology.push(ChronologyRule {
            before: "NOM".into(),
            after: "DATE".into(),
            enabled: true,
        })
    });
    check(|c| {
        c.segmentation = SegmentationRule {
            enabled: true,
            name_column: Some(1),
            bp_column: Some(1),
            ..Default::default()
        }
    });
    check(|c| c.header_row = 0);
    check(|c| c.version = 999);
    check(|c| {
        c.fields[0].required = true;
        c.mappings[0].field_code = None;
    });
}

#[test]
fn invalid_decisions_are_rejected_not_silently_ignored() {
    let (config, source) = fixture();
    for key in [
        cell_key(&config.sheet, 1, 1),
        cell_key(&config.sheet, 2, 6),
        cell_key("Other", 2, 1),
        cell_key(&config.sheet, 99, 1),
        "invalid".into(),
    ] {
        let decisions = Decisions::from([(key, Decision::default())]);
        assert!(analyze(&config, &source, &decisions, &JobControl::default()).is_err());
    }
    for choice in [
        decision("drop_row", CellValue::empty()),
        decision("set", CellValue::typed("formula", "=1+1")),
        decision("set", CellValue::typed("integer", "1.5")),
        decision("set", CellValue::typed("date", "2025-09-31")),
    ] {
        assert!(analyze(
            &config,
            &source,
            &Decisions::from([(cell_key(&config.sheet, 2, 1), choice)]),
            &JobControl::default()
        )
        .is_err());
    }
    assert_eq!(parse_cell_key("A!B!2:3").unwrap(), ("A!B", 2, 3));
    for key in ["S!0:1", "S!01:1", "S!1:0", "S!1:2:3", "!1:1"] {
        assert!(parse_cell_key(key).is_err());
    }
}

#[test]
fn hashes_are_stable_and_report_choices_do_not_invalidate_analysis() {
    let (mut config, _) = fixture();
    let first = config_hash(&config).unwrap();
    config.harmonized = false;
    config.report_collision = "replace".into();
    config.reports.clear();
    assert_eq!(config_hash(&config).unwrap(), first);
    config.rules.insert(
        "TEL".into(),
        FieldRule {
            phone_policy: "first".into(),
            ..Default::default()
        },
    );
    assert_ne!(config_hash(&config).unwrap(), first);
    let key = cell_key(&config.sheet, 2, 1);
    let a = Decisions::from([(key.clone(), Decision::default())]);
    let b = Decisions::from([(key, decision("clear", CellValue::empty()))]);
    assert_ne!(decisions_hash(&a).unwrap(), decisions_hash(&b).unwrap());
    assert_eq!(
        decisions_hash(&a).unwrap(),
        decisions_hash(
            &serde_json::from_str::<Decisions>(&serde_json::to_string(&a).unwrap()).unwrap()
        )
        .unwrap()
    );
    let number = CellValue::typed("decimal", "12345678901234567890.123456789");
    let encoded = serde_json::to_value(&number).unwrap();
    assert!(encoded["value"].is_string());
    assert_eq!(
        serde_json::from_value::<CellValue>(encoded).unwrap(),
        number
    );
}

#[test]
fn progress_is_monotonic_and_cancellation_is_cooperative() {
    let (config, mut source) = fixture();
    source.sheets[0].rows = 305;
    let events = Arc::new(Mutex::new(Vec::<Progress>::new()));
    let captured = events.clone();
    let job = JobControl {
        progress: Some(Arc::new(move |p| captured.lock().unwrap().push(p))),
        ..Default::default()
    };
    analyze(&config, &source, &Decisions::new(), &job).unwrap();
    let events = events.lock().unwrap();
    assert_eq!(events.first().unwrap().current, 0);
    assert_eq!(events.last().unwrap().phase, "complete");
    assert_eq!(events.last().unwrap().current, 304);
    assert!(events.windows(2).all(|p| p[0].current <= p[1].current));
    assert!(events.iter().all(|p| p.total == Some(304)));
    drop(events);
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let job = JobControl {
        cancelled,
        progress: Some(Arc::new(move |p| {
            if p.current >= 100 {
                flag.store(true, Ordering::Relaxed);
            }
        })),
    };
    assert!(matches!(
        analyze(&config, &source, &Decisions::new(), &job),
        Err(FdxError::Cancelled)
    ));
    assert!(matches!(
        analyze(&config, &source, &Decisions::new(), &job),
        Err(FdxError::Cancelled)
    ));
}

#[test]
fn source_hash_is_verified_before_and_after_snapshot_analysis() {
    let (mut config, mut source) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("synthetic-source.bin");
    fs::write(&path, b"synthetic bytes owned by the test").unwrap();
    config.source_path = path.to_string_lossy().into_owned();
    source.source_hash = sha256(&path).unwrap();
    run(&config, &source, &Decisions::new());
    let path_for_callback = path.clone();
    let job = JobControl {
        progress: Some(Arc::new(move |p| {
            if p.phase == "analyze" && p.current == 0 {
                fs::write(&path_for_callback, b"modified").unwrap();
            }
        })),
        ..Default::default()
    };
    assert!(analyze(&config, &source, &Decisions::new(), &job)
        .unwrap_err()
        .to_string()
        .contains("source a change"));
    assert!(analyze(&config, &source, &Decisions::new(), &JobControl::default()).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"modified");
}

#[test]
fn source_dimensions_and_empty_selection_are_rejected() {
    let (config, mut source) = fixture();
    source.sheets[0].rows = 1;
    assert!(analyze(&config, &source, &Decisions::new(), &JobControl::default()).is_err());
    source.sheets[0].rows = 1_000_000;
    source.sheets[0].columns = 10;
    assert!(analyze(&config, &source, &Decisions::new(), &JobControl::default()).is_err());
    source.sheets[0].rows = 3;
    source.sheets[0].columns = 7;
    source.sheets[0]
        .cells
        .insert((10, 1), CellValue::text("outside dimensions"));
    assert!(analyze(&config, &source, &Decisions::new(), &JobControl::default()).is_err());
}

proptest! {
    #[test]
    fn whitespace_cleanup_is_idempotent(text in any::<String>()) {
        prop_assert_eq!(clean_spaces(&clean_spaces(&text)), clean_spaces(&text));
    }

    #[test]
    fn exact_decimal_integer_roundtrip(value in -999_999_999_999_999i64..=999_999_999_999_999i64) {
        let parsed = parse_number(&CellValue::text(value.to_string()), &FieldRule::default(), true).unwrap();
        prop_assert_eq!(parsed, decimal(&value.to_string()).unwrap());
    }

    #[test]
    fn international_number_never_loses_digits(digits in "[0-9]{1,30}") {
        let number = format!("+{digits}");
        prop_assert_eq!(split_phones(&number), vec![number]);
    }
}
