use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::{
    models::{CellValue, ColumnMapping, DictionaryImport, FieldSpec},
    transforms::decimal,
};

pub fn normalize(text: &str) -> String {
    let folded: String = text
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            '\u{153}' => "oe".to_owned(),
            '\u{e6}' => "ae".to_owned(),
            c if c.is_ascii_alphanumeric() => c.to_string(),
            _ => " ".into(),
        })
        .collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn infer_field(code: &str, label: &str, dtype: &str, size: &str) -> FieldSpec {
    let combined = normalize(&format!("{code} {label}"));
    let words: Vec<_> = combined.split_whitespace().collect();
    let type_text = normalize(dtype);
    let dtype = if type_text.contains("alphanumerique")
        || ["texte", "string", "text"]
            .iter()
            .any(|v| type_text.contains(v))
    {
        "text"
    } else if type_text.contains("date") || combined.starts_with("date ") || combined == "date" {
        "date"
    } else if type_text
        .split_whitespace()
        .any(|word| ["entier", "integer", "int"].contains(&word))
    {
        "integer"
    } else if [
        "numerique",
        "decimal",
        "number",
        "float",
        "double",
        "montant",
    ]
    .iter()
    .any(|v| type_text.contains(v))
    {
        "decimal"
    } else if type_text.contains("bool") {
        "boolean"
    } else {
        "text"
    };
    let role = if words
        .iter()
        .any(|v| ["tel", "telephone", "phone"].contains(v))
    {
        "phone"
    } else if words
        .iter()
        .any(|v| ["niu", "nui", "identifiant", "identifier", "rccm", "rib"].contains(v))
    {
        "identifier"
    } else if words.iter().any(|v| ["nom", "name"].contains(v))
        && (words.contains(&"entreprise") || combined.contains("en sel"))
    {
        "company"
    } else {
        "standard"
    };
    let dtype = if matches!(role, "phone" | "identifier") {
        "text"
    } else {
        dtype
    };
    let mut field = FieldSpec {
        code: code.trim().to_owned(),
        label: label.trim().trim_matches('"').to_owned(),
        dtype: dtype.into(),
        role: role.into(),
        max_length: size.trim().parse().ok(),
        ..Default::default()
    };
    if dtype == "date" && !size.trim().is_empty() && field.max_length.is_none() {
        field.date_format = canonical_date_format(size);
    }
    field
}

fn canonical_date_format(value: &str) -> String {
    value
        .trim()
        .to_uppercase()
        .replace("AAAA", "YYYY")
        .replace("JJ", "DD")
        .replace("AA", "YY")
}

fn header_key(value: &str) -> Option<&'static str> {
    Some(match normalize(value).as_str() {
        "code" | "code du champ" | "nom technique" | "field code" => "code",
        "designation du champ" | "designation" | "libelle" | "champ" | "nom" | "field name" => {
            "label"
        }
        "type de donnees" | "type de donnee" | "type" | "datatype" | "data type" => "dtype",
        "taille" | "taille max" | "longueur" | "longueur max" | "size" | "max length" => "size",
        "obligatoire" | "required" | "requis" => "required",
        "motif" | "pattern" | "regex" => "pattern",
        "valeurs autorisees" | "valeurs" | "enum" | "allowed values" => "allowed",
        "minimum" | "min" => "minimum",
        "maximum" | "max" => "maximum",
        "role" | "fonction" => "role",
        "format" | "format date" | "format de date" | "date format" => "date_format",
        _ => return None,
    })
}

// A small delimited-record reader: quoted delimiters/doubled quotes and escaped
// Markdown pipes are structural; all other backslashes belong to the field.
fn record(line: &str, delimiter: char) -> Option<Vec<String>> {
    let line = if delimiter == '|' {
        let line = line.trim();
        let line = line.strip_prefix('|').unwrap_or(line);
        line.strip_suffix('|').unwrap_or(line)
    } else {
        line
    };
    let mut chars = line.chars().peekable();
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    quoted = false;
                }
            } else {
                current.push(c);
            }
        } else if c == '"' && current.trim().is_empty() {
            current.clear();
            quoted = true;
        } else if delimiter == '|' && c == '\\' && chars.peek() == Some(&'|') {
            chars.next();
            current.push('|');
        } else if c == delimiter {
            cells.push(current.trim().to_owned());
            current.clear();
        } else {
            current.push(c);
        }
    }
    if quoted {
        return None;
    }
    cells.push(current.trim().to_owned());
    Some(cells)
}

fn headers(cells: &[String]) -> Option<BTreeMap<&'static str, usize>> {
    let found: BTreeMap<_, _> = cells
        .iter()
        .enumerate()
        .filter_map(|(i, cell)| header_key(cell).map(|k| (k, i)))
        .collect();
    (found.contains_key("label") && (found.contains_key("code") || found.contains_key("dtype")))
        .then_some(found)
}

pub fn parse_dictionary(text: &str) -> DictionaryImport {
    let mut result = DictionaryImport::default();
    let mut table: Option<(char, BTreeMap<&str, usize>, usize)> = None;
    let mut pending = String::new();
    let mut seen = BTreeSet::new();
    for raw in text.trim_start_matches('\u{feff}').lines() {
        let line = raw.trim_matches(' ');
        if pending.is_empty()
            && (line.trim().is_empty() || line.starts_with('#') || line.starts_with("```"))
        {
            continue;
        }
        if pending.is_empty() && line.chars().all(|c| c.is_whitespace() || "|:-".contains(c)) {
            continue;
        }
        if pending.is_empty() {
            let mut found_header = false;
            for delimiter in ['\t', '|', ';', ','] {
                if let Some(cells) = record(line, delimiter) {
                    if let Some(found) = headers(&cells) {
                        for cell in &cells {
                            if header_key(cell).is_none()
                                && !matches!(normalize(cell).as_str(), "no" | "n" | "numero" | "")
                            {
                                result.notes.push(format!(
                                    "Colonne du dictionnaire non interpretee : {cell}"
                                ));
                            }
                        }
                        table = Some((delimiter, found, cells.len()));
                        found_header = true;
                        break;
                    }
                }
            }
            if found_header {
                continue;
            }
        }
        let mut field = if let Some((delimiter, ref headers, width)) = table {
            if !pending.is_empty() {
                pending.push('\n');
            }
            pending.push_str(line);
            let Some(mut cells) = record(&pending, delimiter) else {
                continue;
            };
            let original_line = std::mem::take(&mut pending);
            if cells.len() > width || cells.len() == 1 {
                result.notes.push(format!(
                    "Ligne de dictionnaire a completer (colonnes incoherentes) : {original_line}"
                ));
                continue;
            }
            cells.resize(width, String::new());
            let value = |key: &str| {
                headers
                    .get(key)
                    .and_then(|&i| cells.get(i))
                    .map(String::as_str)
                    .unwrap_or("")
            };
            if value("label").is_empty() {
                result.notes.push(original_line);
                continue;
            }
            let code = if value("code").is_empty() {
                normalize(value("label")).replace(' ', "_").to_uppercase()
            } else {
                value("code").to_owned()
            };
            let mut spec = infer_field(&code, value("label"), value("dtype"), value("size"));
            if !value("dtype").is_empty()
                && ![
                    "alpha", "text", "string", "date", "numer", "decimal", "number", "float",
                    "double", "entier", "int", "bool", "montant",
                ]
                .iter()
                .any(|word| normalize(value("dtype")).contains(word))
            {
                result.notes.push(format!(
                    "Type non reconnu, stockage texte a confirmer : {code}, {}",
                    value("dtype")
                ));
            }
            let required = normalize(value("required"));
            spec.required = matches!(
                required.as_str(),
                "oui" | "yes" | "true" | "1" | "obligatoire"
            );
            if !matches!(
                required.as_str(),
                "" | "oui"
                    | "yes"
                    | "true"
                    | "1"
                    | "obligatoire"
                    | "non"
                    | "no"
                    | "false"
                    | "0"
                    | "facultatif"
            ) {
                result.notes.push(format!(
                    "Caractere obligatoire a confirmer : {code}, {}",
                    value("required")
                ));
            }
            spec.pattern = value("pattern").to_owned();
            if !spec.pattern.is_empty() && fancy_regex::Regex::new(&spec.pattern).is_err() {
                result
                    .notes
                    .push(format!("Motif invalide a corriger : {code}"));
            }
            spec.allowed_values = value("allowed")
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect();
            for (key, target) in [
                ("minimum", &mut spec.minimum),
                ("maximum", &mut spec.maximum),
            ] {
                if !value(key).is_empty() {
                    let bound = value(key).replace(',', ".");
                    if decimal(&bound).is_err() {
                        result
                            .notes
                            .push(format!("Borne a confirmer : {code}, {}", value(key)));
                    }
                    *target = Some(bound);
                }
            }
            if !value("role").is_empty() {
                spec.role = match normalize(value("role")).as_str() {
                    "standard" => "standard".into(),
                    "phone" | "telephone" | "tel" => "phone".into(),
                    "identifier" | "identifiant" => "identifier".into(),
                    "company" | "entreprise" => "company".into(),
                    _ => {
                        result
                            .notes
                            .push(format!("Role a confirmer : {code}, {}", value("role")));
                        value("role").into()
                    }
                };
                if matches!(spec.role.as_str(), "phone" | "identifier") {
                    spec.dtype = "text".into();
                }
            }
            if !value("date_format").is_empty() {
                spec.date_format = canonical_date_format(value("date_format"));
            }
            if !value("size").is_empty()
                && (spec.max_length == Some(0)
                    || (spec.max_length.is_none() && spec.dtype != "date"))
            {
                result
                    .notes
                    .push(format!("Longueur a confirmer : {code}, {}", value("size")));
            }
            spec
        } else {
            let stripped = line.trim_start_matches(['-', '*']).trim();
            if let Some((code, details)) = stripped.split_once(':') {
                let kind = normalize(details);
                if !code.trim().is_empty()
                    && [
                        "date",
                        "numerique",
                        "texte",
                        "alphanum",
                        "entier",
                        "decimal",
                        "boolean",
                    ]
                    .iter()
                    .any(|word| kind.contains(word))
                {
                    let size = details
                        .split_whitespace()
                        .find(|word| word.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or("");
                    result
                        .notes
                        .push(format!("Definition libre a confirmer : {line}"));
                    infer_field(code, code, details, size)
                } else {
                    result.notes.push(line.to_owned());
                    continue;
                }
            } else {
                result.notes.push(line.to_owned());
                continue;
            }
        };
        field.code = field.code.trim().to_owned();
        if !seen.insert(field.code.clone()) {
            result
                .notes
                .push(format!("Code du dictionnaire duplique : {}", field.code));
        }
        result.fields.push(field);
    }
    if !pending.is_empty() {
        result.notes.push(format!(
            "Guillemets non fermes, ligne a completer : {pending}"
        ));
    }
    if result.fields.is_empty() {
        result.notes.insert(
            0,
            "Aucun tableau reconnu : ajoutez les champs dans le formulaire.".into(),
        );
    }
    result
}

fn matching_words(value: &str) -> String {
    normalize(value)
        .split_whitespace()
        .flat_map(|word| match word {
            "mo" => vec!["maitre", "ouvrage"],
            "mod" => vec!["maitre", "ouvrage", "delegue"],
            "de" | "du" | "des" | "le" | "la" | "l" | "d" | "en" | "et" => vec![],
            _ => vec![word],
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn similarity(left: &str, right: &str) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let ratio = rapidfuzz::fuzz::ratio(left.chars(), right.chars()) * 100.0;
    let sorted = |value: &str| {
        let mut words: Vec<_> = value.split_whitespace().collect();
        words.sort_unstable();
        words.join(" ")
    };
    let tokens = rapidfuzz::fuzz::ratio(sorted(left).chars(), sorted(right).chars()) * 95.0;
    let (short, long) = if left.len() < right.len() {
        (left, right)
    } else {
        (right, left)
    };
    let partial = if short.len() >= 3
        && long
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(short.split_whitespace().count())
            .any(|window| window.join(" ") == short)
    {
        90.0
    } else {
        0.0
    };
    ratio.max(tokens).max(partial)
}

pub fn suggest_mappings(headers: &[CellValue], fields: &[FieldSpec]) -> Vec<ColumnMapping> {
    let mut proposals: Vec<_> = (1..=headers.len())
        .map(|column| ColumnMapping {
            column: column as u32,
            ..Default::default()
        })
        .collect();
    let mut pairs = Vec::new();
    for (position, header) in headers.iter().enumerate() {
        let query = matching_words(&header.value);
        if header.is_empty() || query.is_empty() {
            continue;
        }
        let inferred = infer_field("", &header.value, "", "");
        for spec in fields {
            let label = matching_words(&spec.label);
            let mut score = if normalize(&header.value) == normalize(&spec.code)
                || normalize(&header.value) == normalize(&spec.label)
                || query == label
            {
                100.0
            } else {
                similarity(&query, &label)
            };
            if score < 100.0 {
                if matches!(inferred.role.as_str(), "phone" | "identifier") {
                    score = if inferred.role == spec.role {
                        (score + 20.0).min(99.0)
                    } else {
                        score * 0.5
                    };
                }
                if !label.is_empty()
                    && query.split_whitespace().next() == label.split_whitespace().next()
                {
                    score = (score + 8.0).min(99.0);
                }
            }
            pairs.push((score, position, &spec.code));
        }
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
    let mut taken_fields = BTreeSet::new();
    for (score, position, code) in pairs {
        if score < 55.0 || proposals[position].field_code.is_some() || !taken_fields.insert(code) {
            continue;
        }
        proposals[position].field_code = Some(code.clone());
        proposals[position].score = (score * 10.0).round() / 10.0;
    }
    proposals
}
