use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    str::FromStr,
};

use bigdecimal::BigDecimal;
use fancy_regex::{Regex, RegexBuilder};
use sha2::{Digest, Sha256};

use crate::{
    models::*,
    transforms::{
        clean_spaces, date_cell, date_options, decimal, iso_date, number_cell, parse_number,
        segment_company, split_phones_region, valid_phone, NumberParser,
    },
};

fn pattern_regex(pattern: &str) -> Result<Option<Regex>> {
    if pattern.is_empty() {
        return Ok(None);
    }
    if pattern.len() > 16_384 {
        return Err(error("Motif trop long (limite 16384 octets)."));
    }
    RegexBuilder::new(&format!(r"\A(?:{pattern})\z"))
        .backtrack_limit(100_000)
        .build()
        .map(Some)
        .map_err(|e| error(format!("Motif du dictionnaire invalide : {e}")))
}

fn pattern_matches(pattern: &Regex, value: &CellValue) -> Result<bool> {
    pattern.is_match(&value.display()).map_err(|_| {
        error("Motif trop complexe pour cette valeur ; verification manuelle requise.")
    })
}

pub fn validate_config(config: &ProjectConfig) -> Result<()> {
    if !matches!(config.version, 1 | 2) {
        return Err(error("Version de configuration non prise en charge."));
    }
    if config.sheet.trim().is_empty() {
        return Err(error("Choisissez la feuille a analyser."));
    }
    if !(1..=1_048_576).contains(&config.header_row) {
        return Err(error("Ligne d'en-tete Excel invalide."));
    }
    let mut fields = BTreeMap::new();
    for field in &config.fields {
        if field.code.trim().is_empty()
            || field.code.trim() != field.code
            || fields.insert(field.code.as_str(), field).is_some()
        {
            return Err(error("Le dictionnaire doit contenir des codes non vides et uniques, sans espaces en bordure."));
        }
        if field.label.trim().is_empty() {
            return Err(error(format!("Libelle manquant : {}", field.code)));
        }
        if !matches!(
            field.dtype.as_str(),
            "text" | "integer" | "decimal" | "date" | "boolean"
        ) {
            return Err(error(format!("Type de champ inconnu : {}", field.code)));
        }
        if !matches!(
            field.role.as_str(),
            "standard" | "phone" | "identifier" | "company"
        ) {
            return Err(error(format!("Role de champ inconnu : {}", field.code)));
        }
        if matches!(field.role.as_str(), "phone" | "identifier") && field.dtype != "text" {
            return Err(error(format!(
                "Stockage texte requis pour le telephone ou l'identifiant : {}",
                field.code
            )));
        }
        if field.max_length == Some(0) {
            return Err(error(format!(
                "La longueur doit etre positive : {}",
                field.code
            )));
        }
        if field.dtype == "date" && field.date_format.trim().is_empty() {
            return Err(error(format!("Format de date manquant : {}", field.code)));
        }
        pattern_regex(&field.pattern)?;
        let minimum = field.minimum.as_deref().map(decimal).transpose()?;
        let maximum = field.maximum.as_deref().map(decimal).transpose()?;
        if let (Some(minimum), Some(maximum)) = (&minimum, &maximum) {
            if minimum > maximum {
                return Err(error(format!("Bornes inversees : {}", field.code)));
            }
        }
    }
    if fields.is_empty() {
        return Err(error("Le dictionnaire ne contient aucun champ."));
    }
    let mut positions = BTreeSet::new();
    let mut mapped = BTreeSet::new();
    for mapping in &config.mappings {
        if !(1..=16_384).contains(&mapping.column) || !positions.insert(mapping.column) {
            return Err(error(
                "Une colonne Excel doit etre valide et ne peut etre associee qu'une seule fois.",
            ));
        }
        if !mapping.score.is_finite() || !(0.0..=100.0).contains(&mapping.score) {
            return Err(error("Score d'association invalide."));
        }
        if let Some(code) = &mapping.field_code {
            if !fields.contains_key(code.as_str()) || !mapping.confirmed {
                return Err(error(
                    "Validez toutes les associations actives vers des champs existants.",
                ));
            }
            if !mapped.insert(code.as_str()) {
                return Err(error(
                    "Un champ du dictionnaire ne peut pas correspondre a deux colonnes.",
                ));
            }
        }
    }
    if mapped.is_empty() {
        return Err(error("Associez au moins une colonne au dictionnaire."));
    }
    for field in &config.fields {
        if field.required && !mapped.contains(field.code.as_str()) {
            return Err(error(format!(
                "Champ obligatoire sans colonne : {}",
                field.code
            )));
        }
    }
    for (code, rule) in &config.rules {
        if !fields.contains_key(code.as_str()) {
            return Err(error(format!("Regle pour un champ inconnu : {code}")));
        }
        validate_rule(rule)?;
    }
    for relation in config.chronology.iter().filter(|r| r.enabled) {
        if !mapped.contains(relation.before.as_str()) || !mapped.contains(relation.after.as_str()) {
            return Err(error(
                "Une regle de chronologie utilise un champ sans colonne.",
            ));
        }
        if relation.before == relation.after
            || fields[relation.before.as_str()].dtype != "date"
            || fields[relation.after.as_str()].dtype != "date"
        {
            return Err(error(
                "La chronologie doit comparer deux champs de dates distincts.",
            ));
        }
    }
    if config.segmentation.enabled {
        let seg = &config.segmentation;
        let targets: Vec<_> = segmentation_targets(seg)
            .into_iter()
            .filter_map(|(_, c)| c)
            .collect();
        if targets.is_empty()
            || targets.iter().copied().collect::<BTreeSet<_>>().len() != targets.len()
        {
            return Err(error(
                "Choisissez des colonnes de destination distinctes pour la segmentation.",
            ));
        }
        if !std::iter::once(seg.source_column)
            .chain(targets)
            .all(|c| (1..=16_384).contains(&c))
        {
            return Err(error("Coordonnees de segmentation invalides."));
        }
    }
    if !matches!(config.report_collision.as_str(), "suffix" | "replace") {
        return Err(error("Politique de collision des rapports inconnue."));
    }
    let mut reports = BTreeSet::new();
    if config
        .reports
        .iter()
        .any(|r| !matches!(r.as_str(), "control" | "original" | "verify") || !reports.insert(r))
    {
        return Err(error("Selection des rapports invalide."));
    }
    Ok(())
}

fn validate_rule(rule: &FieldRule) -> Result<()> {
    if !matches!(rule.decimal_separator.as_str(), "." | ",") {
        return Err(error(
            "Le separateur decimal doit etre un point ou une virgule.",
        ));
    }
    if rule.thousands_separator.chars().count() > 1
        || rule.thousands_separator == rule.decimal_separator
        || rule
            .thousands_separator
            .chars()
            .any(|c| c.is_alphanumeric() || matches!(c, '+' | '-'))
    {
        return Err(error(
            "Separateur de groupement invalide ou identique au separateur decimal.",
        ));
    }
    if !matches!(rule.date_order.as_str(), "dmy" | "mdy" | "ask") {
        return Err(error("Ordre de date inconnu."));
    }
    if phonenumber::country::Id::from_str(&rule.phone_region).is_err() {
        return Err(error(format!(
            "Region telephonique inconnue : {}",
            rule.phone_region
        )));
    }
    if !matches!(
        rule.phone_policy.as_str(),
        "report" | "first" | "first_valid" | "all" | "clear"
    ) {
        return Err(error("Politique telephone inconnue."));
    }
    if rule.phone_policy == "all"
        && (rule.phone_separator.trim().is_empty()
            || rule
                .phone_separator
                .chars()
                .any(|c| c.is_ascii_digit() || c == '+'))
    {
        return Err(error(
            "Choisissez un separateur non numerique explicite entre les telephones.",
        ));
    }
    if !matches!(rule.length_policy.as_str(), "report" | "truncate" | "clear") {
        return Err(error("Politique de longueur inconnue."));
    }
    if !matches!(rule.invalid_policy.as_str(), "report" | "clear") {
        return Err(error("Politique de motif invalide inconnue."));
    }
    if rule.currency_tokens.iter().any(|token| {
        token.trim().is_empty()
            || token
                .chars()
                .any(|c| c.is_ascii_digit() || matches!(c, '+' | '-'))
    }) {
        return Err(error("Unite monetaire invalide."));
    }
    Ok(())
}

fn hash_json(value: &serde_json::Value) -> Result<String> {
    fn visit(value: &serde_json::Value, digest: &mut Sha256) -> Result<()> {
        match value {
            serde_json::Value::Object(object) => {
                digest.update(b"{");
                for (index, (key, value)) in object
                    .iter()
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .enumerate()
                {
                    if index > 0 {
                        digest.update(b",");
                    }
                    digest.update(serde_json::to_vec(key)?);
                    digest.update(b":");
                    visit(value, digest)?;
                }
                digest.update(b"}");
            }
            serde_json::Value::Array(values) => {
                digest.update(b"[");
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        digest.update(b",");
                    }
                    visit(value, digest)?;
                }
                digest.update(b"]");
            }
            _ => digest.update(serde_json::to_vec(value)?),
        }
        Ok(())
    }
    let mut digest = Sha256::new();
    visit(value, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}

pub fn config_hash(config: &ProjectConfig) -> Result<String> {
    let mut value = serde_json::to_value(config)?;
    if let Some(object) = value.as_object_mut() {
        for key in ["harmonized", "report_collision", "reports"] {
            object.remove(key);
        }
    }
    hash_json(&value)
}

pub fn decisions_hash(decisions: &Decisions) -> Result<String> {
    for (key, decision) in decisions {
        parse_cell_key(key)?;
        decision.validate()?;
    }
    hash_json(&serde_json::to_value(decisions)?)
}

struct PreparedField<'a> {
    spec: &'a FieldSpec,
    rule: FieldRule,
    pattern: Option<Regex>,
    minimum: Option<BigDecimal>,
    maximum: Option<BigDecimal>,
    numbers: NumberParser,
}

impl<'a> PreparedField<'a> {
    fn new(spec: &'a FieldSpec, rule: &FieldRule) -> Result<Self> {
        Ok(Self {
            spec,
            rule: rule.clone(),
            pattern: pattern_regex(&spec.pattern)?,
            numbers: NumberParser::new(rule)?,
            minimum: spec.minimum.as_deref().map(decimal).transpose()?,
            maximum: spec.maximum.as_deref().map(decimal).transpose()?,
        })
    }
}

fn text_storage(value: &CellValue) -> String {
    if value.is_numeric() {
        if let Ok(number) = decimal(&value.value) {
            if number.is_integer() {
                return number.with_scale(0).to_string();
            }
        }
    }
    value.display()
}

fn converted_prepared(value: &CellValue, field: &PreparedField<'_>) -> Result<(CellValue, String)> {
    if value.is_blank() || matches!(value.kind.as_str(), "formula" | "error") {
        return Ok((value.clone(), String::new()));
    }
    let spec = field.spec;
    let rule = &field.rule;
    let mut current = value.clone();
    let mut reasons = Vec::new();
    if current.kind == "text" {
        if let Some(replacement) = rule.replacements.get(&current.value) {
            current.value = replacement.clone();
            reasons.push("Remplacement parametre");
        }
        if rule.normalize_spaces {
            let cleaned = if spec.role == "phone" {
                current
                    .value
                    .lines()
                    .map(clean_spaces)
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                clean_spaces(&current.value)
            };
            if cleaned != current.value {
                reasons.push("Espaces normalises");
                current.value = cleaned;
            }
        }
    }
    if rule.convert && !current.is_blank() {
        if spec.role == "phone" {
            let phones = split_phones_region(&text_storage(&current), &rule.phone_region);
            if phones.len() > 1 {
                let replacement = match rule.phone_policy.as_str() {
                    "first" => phones.first().cloned().map(CellValue::text),
                    "first_valid" => phones
                        .iter()
                        .find(|p| valid_phone(p, &rule.phone_region))
                        .cloned()
                        .map(CellValue::text),
                    "all" => Some(CellValue::text(phones.join(&rule.phone_separator))),
                    "clear" => Some(CellValue::empty()),
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    current = replacement;
                    reasons.push("Politique de numeros multiples");
                }
            } else if let Some(phone) = phones
                .first()
                .filter(|p| valid_phone(p, &rule.phone_region))
            {
                current = CellValue::text(phone);
            }
        } else if spec.role == "identifier" || spec.dtype == "text" {
            current = CellValue::text(text_storage(&current));
        } else if matches!(spec.dtype.as_str(), "integer" | "decimal") {
            if let Ok(number) = field.numbers.parse(&current, spec.dtype == "integer") {
                current = number_cell(&number, spec.dtype == "integer");
            }
        } else if spec.dtype == "date" {
            if let Some(date) = date_options(&current, &rule.date_order).parsed {
                current = date_cell(date);
            }
        } else if spec.dtype == "boolean" {
            let input = if current.is_numeric() {
                text_storage(&current)
            } else {
                current.value.clone()
            };
            match input.trim().to_lowercase().as_str() {
                "oui" | "yes" | "true" | "1" => current = CellValue::typed("boolean", "true"),
                "non" | "no" | "false" | "0" => current = CellValue::typed("boolean", "false"),
                _ => {}
            }
        }
    }
    if current.kind == "text" {
        if let Some(length) = spec
            .max_length
            .filter(|&length| current.value.chars().count() > length)
        {
            match rule.length_policy.as_str() {
                "truncate" => {
                    current.value = current.value.chars().take(length).collect();
                    reasons.push("Troncature explicitement parametree");
                }
                "clear" => {
                    current = CellValue::empty();
                    reasons.push("Valeur trop longue videe");
                }
                _ => {}
            }
        }
    }
    if !current.is_blank() && rule.invalid_policy == "clear" {
        if let Some(pattern) = &field.pattern {
            // A pattern execution limit is never evidence of non-conformance.
            if matches!(pattern_matches(pattern, &current), Ok(false)) {
                current = CellValue::empty();
                reasons.push("Valeur non conforme au motif videe");
            }
        }
    }
    if !value.equivalent(&current) && reasons.is_empty() {
        reasons.push("Conversion de type validee dans les parametres");
    }
    Ok((current, reasons.join("; ")))
}

pub fn converted(
    value: &CellValue,
    spec: &FieldSpec,
    rule: &FieldRule,
) -> Result<(CellValue, String)> {
    validate_rule(rule)?;
    converted_prepared(value, &PreparedField::new(spec, rule)?)
}

pub fn manual_value(
    decision: &Decision,
    raw: &CellValue,
    spec: Option<&FieldSpec>,
    rule: &FieldRule,
) -> Result<CellValue> {
    decision.validate()?;
    if raw.is_formula() {
        return Ok(raw.clone());
    }
    match decision.action.as_str() {
        "keep" => return Ok(raw.clone()),
        "clear" => return Ok(CellValue::empty()),
        _ => {}
    }
    let mut value = decision.value.clone();
    if let Some(spec) = spec {
        if !value.is_blank() {
            if spec.dtype == "date" {
                if let Some(date) = date_options(&value, &rule.date_order).parsed {
                    value = date_cell(date);
                }
            } else if matches!(spec.dtype.as_str(), "integer" | "decimal")
                && spec.role == "standard"
            {
                if let Ok(number) = parse_number(&value, rule, spec.dtype == "integer") {
                    value = number_cell(&number, spec.dtype == "integer");
                }
            } else if spec.dtype == "text" || spec.role == "identifier" || spec.role == "phone" {
                value = CellValue::text(text_storage(&value));
            }
        }
    }
    Ok(value)
}

fn finding(
    sheet: &str,
    row: u32,
    col: u32,
    code: &str,
    original: &CellValue,
    current: &CellValue,
    kind: &str,
    message: impl Into<String>,
) -> Finding {
    Finding {
        key: cell_key(sheet, row, col),
        row,
        column: col,
        field_code: code.into(),
        kind: kind.into(),
        message: message.into(),
        original: original.clone(),
        current: current.clone(),
        proposed: None,
        choices: Vec::new(),
    }
}

fn phones_for_validation(value: &CellValue, rule: &FieldRule) -> Vec<String> {
    let text = text_storage(value);
    let text = if rule.phone_policy == "all" && !rule.phone_separator.is_empty() {
        text.replace(&rule.phone_separator, "/")
    } else {
        text
    };
    split_phones_region(&text, &rule.phone_region)
}

fn validate_prepared(
    sheet: &str,
    row: u32,
    col: u32,
    original: &CellValue,
    value: &CellValue,
    field: &PreparedField<'_>,
    allow_multiple: bool,
) -> Vec<Finding> {
    let spec = field.spec;
    let rule = &field.rule;
    let mut findings = Vec::new();
    let mut add =
        |kind: &str, message: String, proposed: Option<CellValue>, choices: Vec<CellValue>| {
            let mut item = finding(sheet, row, col, &spec.code, original, value, kind, message);
            item.proposed = proposed;
            item.choices = choices;
            findings.push(item);
        };
    if value.is_formula() {
        add(
            "formula",
            "Formule conservee ; son resultat sera recalcule dans Excel.".into(),
            None,
            vec![],
        );
        return findings;
    }
    if value.is_blank() {
        if spec.required {
            add(
                "required",
                "Valeur obligatoire manquante.".into(),
                None,
                vec![],
            );
        }
        return findings;
    }
    if value.kind == "error" {
        add(
            "cell_error",
            "Erreur Excel conservee ; verifier la source.".into(),
            None,
            vec![],
        );
    }
    if spec.role == "phone" {
        let phones = phones_for_validation(value, rule);
        let choices: Vec<_> = phones.iter().map(CellValue::text).collect();
        if phones.len() > 1 && rule.phone_policy != "all" && !allow_multiple {
            add(
                "phone_multiple",
                "Plusieurs numeros dans la cellule.".into(),
                choices.first().cloned(),
                choices.clone(),
            );
        }
        let invalid: Vec<_> = phones
            .iter()
            .filter(|p| !valid_phone(p, &rule.phone_region))
            .cloned()
            .collect();
        if phones.is_empty() || !invalid.is_empty() {
            add(
                "phone_invalid",
                format!(
                    "Numero non reconnu pour la region choisie : {}",
                    invalid.join(", ")
                ),
                None,
                choices,
            );
        }
        if value.kind != "text" {
            add(
                "storage",
                "Stockage texte attendu pour preserver les telephones.".into(),
                Some(CellValue::text(text_storage(value))),
                vec![],
            );
        }
    } else if spec.role == "identifier" || spec.dtype == "text" {
        if value.kind != "text" {
            add(
                "storage",
                "Stockage texte attendu pour preserver les identifiants.".into(),
                Some(CellValue::text(text_storage(value))),
                vec![],
            );
        }
    } else if spec.dtype == "date" {
        if value.kind != "date" || iso_date(&value.value).is_none() {
            let options = date_options(value, &rule.date_order);
            let choices: Vec<_> = options.choices.into_iter().map(date_cell).collect();
            let proposed = options
                .parsed
                .map(date_cell)
                .or_else(|| choices.first().cloned());
            add(
                "date_invalid",
                if options.message.is_empty() {
                    "Conversion en date non appliquee.".into()
                } else {
                    options.message
                },
                proposed,
                choices,
            );
        }
    } else if matches!(spec.dtype.as_str(), "integer" | "decimal") {
        match field.numbers.parse(value, spec.dtype == "integer") {
            Ok(number) if !value.is_numeric() => add(
                "number_invalid",
                "Stockage numerique attendu.".into(),
                Some(number_cell(&number, spec.dtype == "integer")),
                vec![],
            ),
            Err(error) => add("number_invalid", error.to_string(), None, vec![]),
            _ => {}
        }
    } else if spec.dtype == "boolean"
        && !(value.kind == "boolean" && matches!(value.value.as_str(), "true" | "false"))
    {
        add(
            "boolean_invalid",
            "Valeur booleenne attendue.".into(),
            None,
            vec![],
        );
    }
    let displayed = value.display();
    if let Some(length) = spec
        .max_length
        .filter(|&limit| value.kind != "date" && displayed.chars().count() > limit)
    {
        add(
            "length",
            format!(
                "Longueur {} superieure a {length}.",
                displayed.chars().count()
            ),
            Some(CellValue::text(
                displayed.chars().take(length).collect::<String>(),
            )),
            vec![],
        );
    }
    if let Some(pattern) = &field.pattern {
        match pattern_matches(pattern, value) {
            Ok(false) => add(
                "pattern",
                "La valeur ne respecte pas le motif du dictionnaire.".into(),
                None,
                vec![],
            ),
            Err(error) => add("pattern_error", error.to_string(), None, vec![]),
            _ => {}
        }
    }
    if !spec.allowed_values.is_empty() && !spec.allowed_values.contains(&displayed) {
        add(
            "constraint",
            "Contrainte non satisfaite : allowed_values".into(),
            None,
            vec![],
        );
    }
    if field.minimum.is_some() || field.maximum.is_some() {
        let number = if value.is_numeric() || value.kind == "text" {
            decimal(&value.value).ok()
        } else {
            None
        };
        for (name, bound, lower) in [
            ("minimum", &field.minimum, true),
            ("maximum", &field.maximum, false),
        ] {
            if let Some(bound) = bound {
                if number.as_ref().map_or(true, |number| {
                    if lower {
                        number < bound
                    } else {
                        number > bound
                    }
                }) {
                    add(
                        "constraint",
                        format!("Contrainte non satisfaite : {name}"),
                        None,
                        vec![],
                    );
                }
            }
        }
    }
    findings
}

pub fn validate_cell(
    sheet: &str,
    row: u32,
    col: u32,
    original: &CellValue,
    value: &CellValue,
    spec: &FieldSpec,
    rule: &FieldRule,
    allow_multiple: bool,
) -> Result<Vec<Finding>> {
    Ok(validate_prepared(
        sheet,
        row,
        col,
        original,
        value,
        &PreparedField::new(spec, rule)?,
        allow_multiple,
    ))
}

fn segmentation_targets(seg: &SegmentationRule) -> [(&'static str, Option<u32>); 4] {
    [
        ("name", seg.name_column),
        ("bp", seg.bp_column),
        ("phone", seg.phone_column),
        ("location", seg.location_column),
    ]
}

fn check_source(config: &ProjectConfig, source: &WorkbookData, job: &JobControl) -> Result<()> {
    job.check()?;
    if source.source_hash.len() != 64 || !source.source_hash.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(error("Empreinte SHA-256 du classeur absente ou invalide."));
    }
    if !config.source_path.is_empty()
        && crate::workbooks::hash_file(Path::new(&config.source_path), job)? != source.source_hash
    {
        return Err(error(
            "Le fichier source a change depuis sa lecture. Relancez l'analyse.",
        ));
    }
    job.check()
}

pub fn analyze(
    config: &ProjectConfig,
    source: &WorkbookData,
    decisions: &Decisions,
    job: &JobControl,
) -> Result<AnalysisResult> {
    job.check()?;
    validate_config(config)?;
    check_source(config, source, job)?;
    let matching: Vec<_> = source
        .sheets
        .iter()
        .filter(|sheet| sheet.name == config.sheet)
        .collect();
    if matching.len() != 1 {
        return Err(error(
            "La feuille choisie n'existe plus ou son nom est duplique.",
        ));
    }
    let sheet = matching[0];
    if sheet.rows > 1_048_576
        || sheet.columns == 0
        || sheet.columns > 16_384
        || u64::from(sheet.rows) * u64::from(sheet.columns) > 5_000_000
    {
        return Err(error("Limite de cette version : 5 millions de cellules par feuille traitee, dans les dimensions Excel."));
    }
    if config.header_row >= sheet.rows {
        return Err(error("Aucune ligne de donnees apres l'en-tete choisi."));
    }
    if config.mappings.iter().any(|m| m.column > sheet.columns) {
        return Err(error("Une association pointe vers une colonne absente."));
    }
    for (index, (&(row, col), value)) in sheet.cells.iter().enumerate() {
        if index % 256 == 0 {
            job.check()?;
        }
        if row == 0 || col == 0 || row > sheet.rows || col > sheet.columns {
            return Err(error(
                "Le snapshot contient une cellule hors des dimensions declarees.",
            ));
        }
        value.validate()?;
    }
    let specs: BTreeMap<_, _> = config.fields.iter().map(|s| (s.code.as_str(), s)).collect();
    let default_rule = FieldRule::default();
    let mut by_column = BTreeMap::new();
    let mut col_by_code = BTreeMap::new();
    for mapping in &config.mappings {
        if let Some(code) = &mapping.field_code {
            by_column.insert(
                mapping.column,
                PreparedField::new(
                    specs[code.as_str()],
                    config.rules.get(code).unwrap_or(&default_rule),
                )?,
            );
            col_by_code.insert(code.as_str(), mapping.column);
        }
    }
    let mut editable: BTreeSet<_> = by_column.keys().copied().collect();
    let seg = &config.segmentation;
    if seg.enabled {
        if seg.source_column > sheet.columns {
            return Err(error("Colonne de segmentation absente."));
        }
        for (_, column) in segmentation_targets(seg) {
            if let Some(column) = column {
                if column > sheet.columns {
                    return Err(error("La segmentation doit cibler une colonne existante."));
                }
                editable.insert(column);
            }
        }
    }
    let mut decisions_by_row: BTreeMap<u32, BTreeMap<u32, &Decision>> = BTreeMap::new();
    for (key, decision) in decisions {
        job.check()?;
        let (name, row, col) = parse_cell_key(key)?;
        decision.validate()?;
        if name != config.sheet
            || row <= config.header_row
            || row > sheet.rows
            || col > sheet.columns
            || !editable.contains(&col)
        {
            return Err(error(format!(
                "Decision hors du perimetre autorise : {key}"
            )));
        }
        decisions_by_row
            .entry(row)
            .or_default()
            .insert(col, decision);
    }
    let mut result = AnalysisResult {
        source_hash: source.source_hash.clone(),
        config_hash: config_hash(config)?,
        decisions_hash: decisions_hash(decisions)?,
        sheet: config.sheet.clone(),
        row_count: sheet.rows - config.header_row,
        column_count: sheet.columns,
        ..Default::default()
    };
    for field in by_column.values() {
        result.counts_before.insert(field.spec.code.clone(), 0);
        result.filled.insert(field.spec.code.clone(), 0);
        result.maximum_lengths.insert(field.spec.code.clone(), 0);
    }
    job.report(
        "analyze",
        0,
        Some(u64::from(result.row_count)),
        "Analyse de toutes les lignes",
    );
    for row in config.header_row + 1..=sheet.rows {
        job.check()?;
        let row_decisions = decisions_by_row.get(&row);
        let decision_for = |col: u32| {
            row_decisions
                .and_then(|decisions| decisions.get(&col))
                .copied()
        };
        let mut raw_values = BTreeMap::new();
        let mut values = BTreeMap::new();
        let mut reasons = BTreeMap::new();
        let mut conflicts = Vec::new();
        let mut original_source = None;
        for (&col, field) in &by_column {
            job.check()?;
            let raw = sheet.cell(row, col);
            if !validate_prepared(&sheet.name, row, col, &raw, &raw, field, false).is_empty() {
                *result
                    .counts_before
                    .entry(field.spec.code.clone())
                    .or_default() += 1;
            }
            // Exact decisions do not execute (even temporarily) group policies.
            let (value, reason) = if decision_for(col).is_some() {
                (raw.clone(), String::new())
            } else {
                converted_prepared(&raw, field)?
            };
            raw_values.insert(col, raw);
            values.insert(col, value);
            reasons.insert(col, reason);
        }
        if seg.enabled {
            let source_value = sheet.cell(row, seg.source_column);
            if source_value.is_formula() && !values.contains_key(&seg.source_column) {
                result.findings.push(finding(
                    &sheet.name,
                    row,
                    seg.source_column,
                    &format!("COL_{}", seg.source_column),
                    &source_value,
                    &source_value,
                    "formula",
                    "Formule source conservee ; segmentation non appliquee.",
                ));
            } else if !source_value.is_blank()
                && !matches!(source_value.kind.as_str(), "formula" | "error")
            {
                let parts = segment_company(&source_value.display());
                original_source = Some(source_value);
                for (part, target) in segmentation_targets(seg) {
                    let Some(target) = target else {
                        continue;
                    };
                    let Some(text) = parts.get(part).and_then(Option::as_ref) else {
                        continue;
                    };
                    let raw = sheet.cell(row, target);
                    raw_values.entry(target).or_insert_with(|| raw.clone());
                    values.entry(target).or_insert_with(|| raw.clone());
                    if raw.is_formula() || decision_for(target).is_some() {
                        continue;
                    }
                    let proposed = CellValue::text(text);
                    if target != seg.source_column
                        && !raw.is_blank()
                        && !raw.equivalent(&proposed)
                        && !seg.overwrite
                    {
                        conflicts.push((target, proposed));
                    } else {
                        let converted = match by_column.get(&target) {
                            Some(field) => converted_prepared(&proposed, field)?.0,
                            None => proposed,
                        };
                        values.insert(target, converted);
                        reasons.insert(target, "Segmentation validee dans les parametres".into());
                    }
                }
            }
        }
        if let Some(decisions) = row_decisions {
            for (&col, decision) in decisions {
                let raw = raw_values
                    .entry(col)
                    .or_insert_with(|| sheet.cell(row, col));
                let field = by_column.get(&col);
                let value = manual_value(
                    decision,
                    raw,
                    field.map(|f| f.spec),
                    field.map(|f| &f.rule).unwrap_or(&default_rule),
                )?;
                values.insert(col, value);
                reasons.insert(col, decision.reason.clone());
            }
        }
        for (&col, value) in &values {
            let raw = &raw_values[&col];
            let field = by_column.get(&col);
            let code = field
                .map(|f| f.spec.code.clone())
                .unwrap_or_else(|| format!("COL_{col}"));
            if let Some(field) = field {
                result.findings.extend(validate_prepared(
                    &sheet.name,
                    row,
                    col,
                    raw,
                    value,
                    field,
                    decision_for(col).is_some_and(|d| d.allow_multiple),
                ));
                if !value.is_blank() {
                    *result.filled.entry(code.clone()).or_default() += 1;
                    let length = result.maximum_lengths.entry(code.clone()).or_default();
                    *length = (*length).max(value.display().chars().count());
                }
            } else if raw.is_formula() {
                result.findings.push(finding(
                    &sheet.name,
                    row,
                    col,
                    &code,
                    raw,
                    value,
                    "formula",
                    "Formule conservee ; son resultat sera recalcule dans Excel.",
                ));
            }
            if !raw.is_formula() && !raw.equivalent(value) {
                let change = CellChange {
                    key: cell_key(&sheet.name, row, col),
                    row,
                    column: col,
                    field_code: code,
                    before: raw.clone(),
                    after: value.clone(),
                    reason: reasons
                        .get(&col)
                        .cloned()
                        .filter(|r| !r.is_empty())
                        .unwrap_or_else(|| "Traitement valide dans les parametres".into()),
                };
                result.originals.push(change.clone());
                result.changes.push(change);
            }
        }
        for (col, proposed) in conflicts {
            let code = by_column
                .get(&col)
                .map(|f| f.spec.code.clone())
                .unwrap_or_else(|| format!("COL_{col}"));
            let mut issue = finding(
                &sheet.name,
                row,
                col,
                &code,
                &raw_values[&col],
                &values[&col],
                "segmentation_conflict",
                "Destination deja renseignee ; remplacement a confirmer.",
            );
            issue.proposed = Some(proposed);
            result.findings.push(issue);
        }
        if let Some(source_value) = original_source {
            let key = cell_key(&sheet.name, row, seg.source_column);
            if !result
                .originals
                .iter()
                .rev()
                .take_while(|c| c.row == row)
                .any(|c| c.key == key)
            {
                result.originals.push(CellChange {
                    key,
                    row,
                    column: seg.source_column,
                    field_code: by_column
                        .get(&seg.source_column)
                        .map(|f| f.spec.code.clone())
                        .unwrap_or_else(|| format!("COL_{}", seg.source_column)),
                    before: source_value.clone(),
                    after: values
                        .get(&seg.source_column)
                        .cloned()
                        .unwrap_or(source_value),
                    reason: "Source de segmentation conservee".into(),
                });
            }
        }
        for relation in config.chronology.iter().filter(|r| r.enabled) {
            let before_col = col_by_code[relation.before.as_str()];
            let after_col = col_by_code[relation.after.as_str()];
            let before = &values[&before_col];
            let after = &values[&after_col];
            if before.kind == "date" && after.kind == "date" {
                if let (Some(left), Some(right)) = (iso_date(&before.value), iso_date(&after.value))
                {
                    if left > right {
                        result.findings.push(finding(
                            &sheet.name,
                            row,
                            before_col,
                            &relation.before,
                            &raw_values[&before_col],
                            before,
                            "chronology",
                            format!(
                                "{} ({}) apres {} ({}).",
                                relation.before,
                                before.display(),
                                relation.after,
                                after.display()
                            ),
                        ));
                    }
                }
            }
        }
        let processed = row - config.header_row;
        if processed % 100 == 0 || processed == result.row_count {
            job.report(
                "analyze",
                u64::from(processed),
                Some(u64::from(result.row_count)),
                "Analyse et validation de toutes les lignes",
            );
        }
    }
    job.check()?;
    check_source(config, source, job)?;
    job.report(
        "complete",
        u64::from(result.row_count),
        Some(u64::from(result.row_count)),
        "Analyse terminee",
    );
    job.check()?;
    Ok(result)
}
