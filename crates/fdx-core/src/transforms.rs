use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate};
use regex::Regex;
use std::{collections::BTreeMap, str::FromStr, sync::OnceLock};

use crate::{
    dictionary::normalize,
    models::{error, CellValue, FieldRule, Result},
};

pub fn clean_spaces(value: &str) -> String {
    value
        .split(|c: char| c.is_whitespace() || c == '\u{200b}')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse a wire-format decimal without binary floating point or unbounded exponents.
pub fn decimal(value: &str) -> Result<BigDecimal> {
    static NUMBER: OnceLock<Regex> = OnceLock::new();
    let syntax = NUMBER.get_or_init(|| {
        Regex::new(r"^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$")
            .expect("constant number regex")
    });
    if value.len() > 4096 || !syntax.is_match(value) {
        return Err(error("Nombre non interpretable ou non fini."));
    }
    if let Some((_, exponent)) = value.split_once(['e', 'E']) {
        if exponent
            .parse::<i64>()
            .map_or(true, |n| !(-10000..=10000).contains(&n))
        {
            return Err(error(
                "Exposant numerique hors des limites prises en charge.",
            ));
        }
    }
    let number = BigDecimal::from_str(value).map_err(|_| error("Nombre non interpretable."))?;
    if !(-10000..=10000).contains(&number.as_bigint_and_exponent().1) {
        return Err(error(
            "Echelle numerique hors des limites prises en charge.",
        ));
    }
    Ok(number)
}

fn grouping_spaces(value: &str) -> String {
    value.replace(['\u{00a0}', '\u{202f}'], " ")
}

pub fn parse_number(value: &CellValue, rule: &FieldRule, integer: bool) -> Result<BigDecimal> {
    NumberParser::new(rule)?.parse(value, integer)
}

pub(crate) struct NumberParser {
    currency: Vec<Regex>,
    grouping: Option<Regex>,
    thousands: String,
    separator: String,
}

impl NumberParser {
    pub(crate) fn new(rule: &FieldRule) -> Result<Self> {
        if !matches!(rule.decimal_separator.as_str(), "." | ",")
            || rule.decimal_separator == rule.thousands_separator
        {
            return Err(error("Separateurs numeriques invalides."));
        }
        let mut tokens: Vec<_> = rule
            .currency_tokens
            .iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
        tokens.sort_by_key(|s| std::cmp::Reverse(s.len()));
        let mut currency = Vec::new();
        for token in tokens {
            let token = regex::escape(token.trim());
            currency.push(
                Regex::new(&format!(r"(?i)^(?:{token})\s*|\s*(?:{token})$"))
                    .map_err(|e| error(e.to_string()))?,
            );
        }
        let thousands = grouping_spaces(&rule.thousands_separator);
        let grouping = if thousands.is_empty() {
            None
        } else {
            Some(
                Regex::new(&format!(
                    r"^[+-]?[0-9]{{1,3}}(?:{}[0-9]{{3}})+(?:{}[0-9]+)?$",
                    regex::escape(&thousands),
                    regex::escape(&rule.decimal_separator)
                ))
                .map_err(|e| error(e.to_string()))?,
            )
        };
        Ok(Self {
            currency,
            grouping,
            thousands,
            separator: rule.decimal_separator.clone(),
        })
    }

    pub(crate) fn parse(&self, value: &CellValue, integer: bool) -> Result<BigDecimal> {
        let number = if value.is_numeric() {
            decimal(&value.value)?
        } else if value.kind == "text" {
            let mut text = grouping_spaces(value.value.trim());
            // Currency markers may surround a number, never join two digit runs.
            for re in &self.currency {
                text = re.replace_all(&text, "").trim().to_owned();
            }
            if let Some(grouped) = &self.grouping {
                if text.contains(&self.thousands) {
                    if !grouped.is_match(&text) {
                        return Err(error("Groupement des milliers ambigu ou irregulier."));
                    }
                    text = text.replace(&self.thousands, "");
                }
            }
            if self.separator != "." {
                if text.contains('.') {
                    return Err(error("Point inattendu avec le separateur decimal choisi."));
                }
                text = text.replace(&self.separator, ".");
            }
            // Exponential notation is only accepted for already numeric source cells.
            static PLAIN: OnceLock<Regex> = OnceLock::new();
            if !PLAIN
                .get_or_init(|| {
                    Regex::new(r"^[+-]?[0-9]+(?:\.[0-9]+)?$").expect("constant plain number regex")
                })
                .is_match(&text)
            {
                return Err(error("Valeur non numerique avec les separateurs choisis."));
            }
            decimal(&text)?
        } else {
            return Err(error(
                "Stockage numerique attendu ; un booleen ou une date n'est pas un montant.",
            ));
        };
        if integer && !number.is_integer() {
            return Err(error(
                "Un entier est attendu ; aucun arrondi n'a ete applique.",
            ));
        }
        let (coefficient, _) = number.normalized().as_bigint_and_exponent();
        if coefficient.to_string().trim_start_matches('-').len() > 15 {
            return Err(error(
                "Plus de 15 chiffres significatifs : choisir un stockage texte pour Excel.",
            ));
        }
        Ok(number)
    }
}

pub fn number_cell(number: &BigDecimal, integer: bool) -> CellValue {
    if integer && number.is_integer() {
        CellValue::typed("integer", number.with_scale(0).to_string())
    } else {
        CellValue::typed("decimal", number.to_string())
    }
}

pub fn iso_date(value: &str) -> Option<NaiveDate> {
    if value.len() != 10
        || value.as_bytes().get(4) != Some(&b'-')
        || value.as_bytes().get(7) != Some(&b'-')
    {
        return None;
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    (date.year() >= 1 && date.year() <= 9999 && date.format("%Y-%m-%d").to_string() == value)
        .then_some(date)
}

#[derive(Debug, Clone, Default)]
pub struct DateOptions {
    pub parsed: Option<NaiveDate>,
    pub choices: Vec<NaiveDate>,
    pub message: String,
}

fn month(value: &str) -> Option<u32> {
    Some(match value {
        "janvier" | "janv" | "jan" => 1,
        "fevrier" | "fevr" | "fev" => 2,
        "mars" => 3,
        "avril" | "avr" => 4,
        "mai" => 5,
        "juin" => 6,
        "juillet" | "juil" => 7,
        "aout" => 8,
        "septembre" | "sept" | "sep" => 9,
        "octobre" | "oct" => 10,
        "novembre" | "nov" => 11,
        "decembre" | "dec" => 12,
        _ => return None,
    })
}

fn calendar_options(first: u32, second: u32, year_text: &str, order: &str) -> DateOptions {
    let year = match year_text.parse::<i32>() {
        Ok(v) => v,
        Err(_) => return invalid_date(),
    };
    let year = if year_text.len() == 2 {
        2000 + year
    } else {
        year
    };
    let repaired = match year {
        205 | 225 => 2025,
        _ => year,
    };
    if !(1..=9999).contains(&repaired) {
        return invalid_date();
    }
    let orders = match order {
        "dmy" => vec![(first, second)],
        "mdy" => vec![(second, first)],
        "ask" => vec![(first, second), (second, first)],
        _ => return invalid_date(),
    };
    let mut choices = Vec::new();
    let mut exact = Vec::new();
    for (day, month) in orders {
        if let Some(date) = NaiveDate::from_ymd_opt(repaired, month, day) {
            if !choices.contains(&date) {
                choices.push(date);
            }
            if year == repaired && !exact.contains(&date) {
                exact.push(date);
            }
        } else if (1..=31).contains(&day) && (1..=12).contains(&month) {
            if let Some(date) = (28..=31)
                .rev()
                .find_map(|d| NaiveDate::from_ymd_opt(repaired, month, d))
            {
                if !choices.contains(&date) {
                    choices.push(date);
                }
            }
        }
    }
    if choices.len() == 1 && exact == choices {
        DateOptions {
            parsed: choices.first().copied(),
            ..Default::default()
        }
    } else {
        DateOptions {
            parsed: None,
            choices,
            message: "Date ambigue ou correction calendaire a confirmer.".into(),
        }
    }
}

fn invalid_date() -> DateOptions {
    DateOptions {
        message: "Date incomplete, impossible, multiple ou non reconnue.".into(),
        ..Default::default()
    }
}

pub fn date_options(value: &CellValue, order: &str) -> DateOptions {
    if !matches!(value.kind.as_str(), "date" | "text") {
        return invalid_date();
    }
    let text = clean_spaces(&value.value);
    if let Some(date) = iso_date(&text) {
        return DateOptions {
            parsed: Some(date),
            ..Default::default()
        };
    }
    if value.kind == "date" {
        return invalid_date();
    }
    static ISO_PARTS: OnceLock<Regex> = OnceLock::new();
    let iso_parts = ISO_PARTS.get_or_init(|| {
        Regex::new(r"^([0-9]{4})-([0-9]{2})-([0-9]{2})$").expect("constant ISO parts regex")
    });
    if let Some(caps) = iso_parts.captures(&text) {
        return calendar_options(
            caps[3].parse().unwrap_or(0),
            caps[2].parse().unwrap_or(0),
            &caps[1],
            "dmy",
        );
    }
    // Accept an ISO timestamp only if the whole timestamp is valid.
    if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(&text) {
        return DateOptions {
            parsed: Some(timestamp.date_naive()),
            ..Default::default()
        };
    }
    if let Ok(timestamp) = chrono::NaiveDateTime::parse_from_str(&text, "%Y-%m-%dT%H:%M:%S%.f") {
        return DateOptions {
            parsed: Some(timestamp.date()),
            ..Default::default()
        };
    }
    static WORDS: OnceLock<Regex> = OnceLock::new();
    let words = WORDS.get_or_init(|| {
        Regex::new(r"^([0-9]{1,2})(?:er|eme|e)? ([a-z]+) ([0-9]{2,4})$")
            .expect("constant month regex")
    });
    if let Some(caps) = words.captures(&normalize(&text)) {
        if let Some(month) = month(&caps[2]) {
            return calendar_options(caps[1].parse().unwrap_or(0), month, &caps[3], "dmy");
        }
    }
    static NUMERIC: OnceLock<Regex> = OnceLock::new();
    static COMPACT: OnceLock<Regex> = OnceLock::new();
    static SPANS: OnceLock<Regex> = OnceLock::new();
    let numeric = NUMERIC.get_or_init(|| {
        Regex::new(r"^([0-9]{1,2})[/.*_\\\-\s]([0-9]{1,2})[/.*_\\\-\s]([0-9]{2,4})$")
            .expect("constant date regex")
    });
    let compact = COMPACT.get_or_init(|| {
        Regex::new(r"^([0-9]{2})/?([0-9]{2})/?([0-9]{4})$").expect("constant compact date regex")
    });
    if let Some(caps) = numeric.captures(&text).or_else(|| compact.captures(&text)) {
        return calendar_options(
            caps[1].parse().unwrap_or(0),
            caps[2].parse().unwrap_or(0),
            &caps[3],
            order,
        );
    }
    let spans = SPANS.get_or_init(|| {
        Regex::new(r"\b[0-9]{1,2}[/.-][0-9]{1,2}[/.-][0-9]{4}\b").expect("constant date span regex")
    });
    let spans: Vec<_> = spans.find_iter(&text).collect();
    if spans.len() > 1 {
        let mut result = DateOptions {
            message: "Plusieurs dates : choisir une date ou un stockage texte.".into(),
            ..Default::default()
        };
        for span in spans {
            let option = date_options(&CellValue::text(span.as_str()), order);
            for date in option.parsed.into_iter().chain(option.choices) {
                if !result.choices.contains(&date) {
                    result.choices.push(date);
                }
            }
        }
        return result;
    }
    invalid_date()
}

pub fn date_cell(date: NaiveDate) -> CellValue {
    CellValue::typed("date", date.format("%Y-%m-%d").to_string())
}

fn noise_regex() -> &'static Regex {
    static NOISE: OnceLock<Regex> = OnceLock::new();
    NOISE.get_or_init(|| Regex::new(r"(?i)(?:\b(?:NIU|NUI|RIB|RCCM|contribuable|compte|fax|email|e-mail)\b|N[\x{b0}\x{ba}]\s*R\.?C)").expect("constant noise regex"))
}

fn tel_regex() -> &'static Regex {
    static TEL: OnceLock<Regex> = OnceLock::new();
    TEL.get_or_init(|| {
        Regex::new(r"(?i)\b(?:t[e\x{e9}]l(?:[e\x{e9}]phone)?|phone)\b\s*[:.]?\s*")
            .expect("constant telephone regex")
    })
}

pub fn split_phones(value: &str) -> Vec<String> {
    split_phones_region(value, "CM")
}

pub fn split_phones_region(value: &str, region: &str) -> Vec<String> {
    let text = tel_regex().replace_all(value, "");
    let text = noise_regex().split(&text).next().unwrap_or("");
    static JOIN: OnceLock<Regex> = OnceLock::new();
    let join = JOIN.get_or_init(|| Regex::new(r"(?i)\b(?:et|ou)\b").expect("constant join regex"));
    let text = join.replace_all(text, "/");
    let mut numbers = Vec::new();
    for chunk in text.split(['/', ';', ',', '|', '\n', '\r']) {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let digits: String = chunk.chars().filter(char::is_ascii_digit).collect();
        // A leading international prefix is never split or truncated.
        let local_blocks = region == "CM"
            && !chunk.starts_with('+')
            && !digits.starts_with("00")
            && matches!(digits.len(), 18 | 27)
            && (0..digits.len())
                .step_by(9)
                .all(|i| matches!(digits.as_bytes()[i], b'6' | b'2'));
        let candidates = if local_blocks {
            (0..digits.len())
                .step_by(9)
                .map(|i| digits[i..i + 9].to_owned())
                .collect()
        } else if digits.is_empty() {
            vec![chunk.to_owned()]
        } else if chunk.chars().any(|c| c.is_alphabetic())
            || chunk.matches('+').count() > usize::from(chunk.starts_with('+'))
        {
            // Unrecognized suffixes/extensions must remain visible for review.
            vec![chunk.to_owned()]
        } else {
            vec![format!(
                "{}{digits}",
                if chunk.starts_with('+') { "+" } else { "" }
            )]
        };
        for candidate in candidates {
            if !numbers.contains(&candidate) {
                numbers.push(candidate);
            }
        }
    }
    numbers
}

pub fn valid_phone(number: &str, region: &str) -> bool {
    if number.is_empty()
        || number
            .chars()
            .enumerate()
            .any(|(i, c)| !c.is_ascii_digit() && !(i == 0 && c == '+'))
    {
        return false;
    }
    let region = match phonenumber::country::Id::from_str(region) {
        Ok(region) => region,
        Err(_) => return false,
    };
    phonenumber::parse(Some(region), number)
        .map(|phone| phonenumber::is_valid(&phone))
        .unwrap_or(false)
}

pub fn segment_company(value: &str) -> BTreeMap<String, Option<String>> {
    let raw = clean_spaces(value);
    let raw = noise_regex().split(&raw).next().unwrap_or("");
    let tel = tel_regex().find(raw);
    let head = &raw[..tel.as_ref().map_or(raw.len(), |m| m.start())];
    let phone = tel.map(|m| split_phones(&raw[m.end()..]).join("/"));
    static BP: OnceLock<Regex> = OnceLock::new();
    static POSTAL: OnceLock<Regex> = OnceLock::new();
    let bp_re = BP.get_or_init(|| {
        Regex::new(r"(?i)\b(?:B\.?\s*P\b\.?|bo[i\x{ee}]te\s+postale\b)\s*[:.]?\s*")
            .expect("constant BP regex")
    });
    let postal =
        POSTAL.get_or_init(|| Regex::new(r"^([0-9][0-9 ]*)(.*)$").expect("constant postal regex"));
    let mut name = head.to_owned();
    let mut bp = None;
    let mut location = None;
    if let Some(found) = bp_re.find(head) {
        name = head[..found.start()].to_owned();
        let after =
            head[found.end()..].trim_matches(|c: char| c.is_whitespace() || ":;,.=-".contains(c));
        if let Some(caps) = postal.captures(after) {
            bp = Some(caps[1].replace(' ', ""));
            location = Some(caps[2].to_owned());
        } else {
            location = Some(after.to_owned());
        }
    }
    [
        ("name", Some(name)),
        ("bp", bp),
        ("phone", phone),
        ("location", location),
    ]
    .into_iter()
    .map(|(key, value)| {
        (
            key.to_owned(),
            value.and_then(|s| {
                let s = s.trim_matches(|c: char| c.is_whitespace() || ";,.-".contains(c));
                (!s.is_empty()).then(|| s.to_owned())
            }),
        )
    })
    .collect()
}
