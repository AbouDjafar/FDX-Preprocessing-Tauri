use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub type Result<T> = std::result::Result<T, FdxError>;
#[derive(Debug, thiserror::Error)]
pub enum FdxError {
    #[error("{0}")]
    Message(String),
    #[error("Operation annulee. Aucun fichier final n'a ete ecrit.")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
pub fn error(message: impl Into<String>) -> FdxError {
    FdxError::Message(message.into())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CellValue {
    pub kind: String,
    pub value: String,
}
impl CellValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self {
            kind: "text".into(),
            value: value.into(),
        }
    }
    pub fn typed(kind: &str, value: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            value: value.into(),
        }
    }
    pub fn empty() -> Self {
        Self {
            kind: "empty".into(),
            value: String::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.kind == "empty" || self.value.is_empty()
    }
    pub fn is_blank(&self) -> bool {
        self.kind == "empty"
            || (self.kind == "text"
                && self
                    .value
                    .chars()
                    .all(|c| c.is_whitespace() || c == '\u{200b}'))
    }
    pub fn is_numeric(&self) -> bool {
        matches!(self.kind.as_str(), "integer" | "decimal")
    }
    pub fn is_formula(&self) -> bool {
        self.kind == "formula"
    }
    pub fn validate(&self) -> Result<()> {
        match self.kind.as_str() {
            "empty" if self.value.is_empty() => Ok(()),
            "text" | "formula" | "error" => Ok(()),
            "integer" | "decimal" => {
                let number = crate::transforms::decimal(&self.value)?;
                if self.kind == "integer" && !number.is_integer() {
                    return Err(error(
                        "Une valeur integer ne peut pas contenir de fraction.",
                    ));
                }
                Ok(())
            }
            "date" if crate::transforms::iso_date(&self.value).is_some() => Ok(()),
            "boolean" if matches!(self.value.as_str(), "true" | "false") => Ok(()),
            _ => Err(error(format!(
                "Valeur de cellule non conforme au type {}.",
                self.kind
            ))),
        }
    }
    pub fn display(&self) -> String {
        match self.kind.as_str() {
            "empty" => String::new(),
            "boolean" if self.value == "true" => "Oui".into(),
            "boolean" if self.value == "false" => "Non".into(),
            "date" => crate::transforms::iso_date(&self.value)
                .map(|date| date.format("%d/%m/%Y").to_string())
                .unwrap_or_else(|| self.value.clone()),
            _ => self.value.clone(),
        }
    }
    /// Equality for preserved workbook values, never between text and numbers.
    pub fn equivalent(&self, other: &Self) -> bool {
        if self.is_empty() && other.is_empty() {
            return true;
        }
        if self.is_numeric() && other.is_numeric() {
            if let (Ok(left), Ok(right)) = (
                crate::transforms::decimal(&self.value),
                crate::transforms::decimal(&other.value),
            ) {
                return left == right;
            }
        }
        self == other
    }
}
impl Default for CellValue {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldSpec {
    pub code: String,
    pub label: String,
    pub dtype: String,
    pub role: String,
    pub max_length: Option<usize>,
    pub date_format: String,
    pub required: bool,
    pub pattern: String,
    pub allowed_values: Vec<String>,
    pub minimum: Option<String>,
    pub maximum: Option<String>,
}
impl Default for FieldSpec {
    fn default() -> Self {
        Self {
            code: String::new(),
            label: String::new(),
            dtype: "text".into(),
            role: "standard".into(),
            max_length: None,
            date_format: "DD/MM/YYYY".into(),
            required: false,
            pattern: String::new(),
            allowed_values: vec![],
            minimum: None,
            maximum: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColumnMapping {
    pub column: u32,
    pub field_code: Option<String>,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldRule {
    pub normalize_spaces: bool,
    pub convert: bool,
    pub decimal_separator: String,
    pub thousands_separator: String,
    pub currency_tokens: Vec<String>,
    pub date_order: String,
    pub phone_region: String,
    pub phone_policy: String,
    pub phone_separator: String,
    pub length_policy: String,
    pub invalid_policy: String,
    pub replacements: BTreeMap<String, String>,
}
impl Default for FieldRule {
    fn default() -> Self {
        Self {
            normalize_spaces: true,
            convert: true,
            decimal_separator: ",".into(),
            thousands_separator: " ".into(),
            currency_tokens: vec!["FCFA".into(), "XAF".into()],
            date_order: "dmy".into(),
            phone_region: "CM".into(),
            phone_policy: "report".into(),
            phone_separator: "/".into(),
            length_policy: "report".into(),
            invalid_policy: "report".into(),
            replacements: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronologyRule {
    pub before: String,
    pub after: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SegmentationRule {
    pub enabled: bool,
    pub source_column: u32,
    pub name_column: Option<u32>,
    pub bp_column: Option<u32>,
    pub phone_column: Option<u32>,
    pub location_column: Option<u32>,
    pub overwrite: bool,
}
impl Default for SegmentationRule {
    fn default() -> Self {
        Self {
            enabled: false,
            source_column: 1,
            name_column: None,
            bp_column: None,
            phone_column: None,
            location_column: None,
            overwrite: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub version: u32,
    pub source_path: String,
    pub original_path: String,
    pub dictionary_path: String,
    pub sheet: String,
    pub header_row: u32,
    pub fields: Vec<FieldSpec>,
    pub mappings: Vec<ColumnMapping>,
    pub rules: BTreeMap<String, FieldRule>,
    pub chronology: Vec<ChronologyRule>,
    pub segmentation: SegmentationRule,
    pub harmonized: bool,
    pub report_collision: String,
    pub reports: Vec<String>,
}
impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            version: 2,
            source_path: String::new(),
            original_path: String::new(),
            dictionary_path: String::new(),
            sheet: String::new(),
            header_row: 1,
            fields: vec![],
            mappings: vec![],
            rules: BTreeMap::new(),
            chronology: vec![],
            segmentation: SegmentationRule::default(),
            harmonized: true,
            report_collision: "suffix".into(),
            reports: vec!["control".into(), "original".into(), "verify".into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Decision {
    pub action: String,
    pub value: CellValue,
    pub allow_multiple: bool,
    pub reason: String,
}
impl Default for Decision {
    fn default() -> Self {
        Self {
            action: "keep".into(),
            value: CellValue::empty(),
            allow_multiple: false,
            reason: "Decision utilisateur".into(),
        }
    }
}
pub type Decisions = BTreeMap<String, Decision>;
pub fn cell_key(sheet: &str, row: u32, column: u32) -> String {
    format!("{sheet}!{row}:{column}")
}
pub fn parse_cell_key(key: &str) -> Result<(&str, u32, u32)> {
    let invalid = || error("Coordonnees de cellule invalides ; feuille!ligne:colonne attendu.");
    let (sheet, coordinates) = key.rsplit_once('!').ok_or_else(invalid)?;
    let (row, column) = coordinates.split_once(':').ok_or_else(invalid)?;
    let row: u32 = row.parse().map_err(|_| invalid())?;
    let column: u32 = column.parse().map_err(|_| invalid())?;
    if sheet.is_empty() || row == 0 || column == 0 || cell_key(sheet, row, column) != key {
        return Err(invalid());
    }
    Ok((sheet, row, column))
}

impl Decision {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.action.as_str(), "keep" | "set" | "clear") {
            return Err(error(format!("Decision inconnue : {}", self.action)));
        }
        if self.action == "set" {
            self.value.validate()?;
            if matches!(self.value.kind.as_str(), "formula" | "error") {
                return Err(error("Une correction ne peut pas introduire une formule ou une erreur Excel. Utilisez du texte litteral."));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub key: String,
    pub row: u32,
    pub column: u32,
    pub field_code: String,
    pub kind: String,
    pub message: String,
    pub original: CellValue,
    pub current: CellValue,
    pub proposed: Option<CellValue>,
    pub choices: Vec<CellValue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellChange {
    pub key: String,
    pub row: u32,
    pub column: u32,
    pub field_code: String,
    pub before: CellValue,
    pub after: CellValue,
    pub reason: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub source_hash: String,
    pub config_hash: String,
    pub decisions_hash: String,
    pub sheet: String,
    pub row_count: u32,
    pub column_count: u32,
    pub findings: Vec<Finding>,
    pub changes: Vec<CellChange>,
    pub originals: Vec<CellChange>,
    pub counts_before: BTreeMap<String, usize>,
    pub filled: BTreeMap<String, usize>,
    pub maximum_lengths: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default)]
pub struct SheetData {
    pub name: String,
    pub rows: u32,
    pub columns: u32,
    pub cells: BTreeMap<(u32, u32), CellValue>,
}
impl SheetData {
    pub fn cell(&self, row: u32, col: u32) -> CellValue {
        self.cells
            .get(&(row, col))
            .cloned()
            .unwrap_or_else(CellValue::empty)
    }
}
#[derive(Debug, Clone, Default)]
pub struct WorkbookData {
    pub source_hash: String,
    pub sheets: Vec<SheetData>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetInfo {
    pub name: String,
    pub rows: u32,
    pub columns: u32,
    pub headers: Vec<CellValue>,
    pub preview: Vec<Vec<CellValue>>,
    pub suggested_header: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkbookInfo {
    pub path: String,
    pub sheets: Vec<SheetInfo>,
    pub suggested_sheet: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DictionaryImport {
    pub fields: Vec<FieldSpec>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub task_id: String,
    pub phase: String,
    pub current: u64,
    pub total: Option<u64>,
    pub message: String,
}
#[derive(Clone, Default)]
pub struct JobControl {
    pub cancelled: Arc<AtomicBool>,
    pub progress: Option<Arc<dyn Fn(Progress) + Send + Sync>>,
}
impl JobControl {
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(FdxError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn report(&self, phase: &str, current: u64, total: Option<u64>, message: &str) {
        if let Some(callback) = &self.progress {
            callback(Progress {
                task_id: String::new(),
                phase: phase.into(),
                current,
                total,
                message: message.into(),
            });
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedProject {
    pub config: ProjectConfig,
    pub decisions: Decisions,
    pub history: Vec<serde_json::Value>,
    pub ui_state: serde_json::Value,
}
