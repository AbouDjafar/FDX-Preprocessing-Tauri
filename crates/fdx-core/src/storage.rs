use crate::{exporter::validate_cell, models::*, workbooks::*};
use chrono::NaiveDateTime;
use rusqlite::{params, types::ValueRef, Connection, OpenFlags};
use serde_json::Value;
use std::{collections::BTreeMap, fs::{self, OpenOptions}, io::Write, path::Path};

const MAX_JSON: u64 = 32 * 1024 * 1024;
const MAX_DATABASE: u64 = 1024 * 1024 * 1024;

// Requires serde_json's arbitrary_precision feature: Number retains the original
// decimal representation, including legacy decisions and untyped history values.
fn exact_json(bytes: &[u8]) -> Result<Value> {
    if bytes.len() as u64 > MAX_JSON { return Err(error("Donnees JSON trop volumineuses.")); }
    Ok(serde_json::from_slice(bytes)?)
}

fn config_from_json(bytes: &[u8]) -> Result<(ProjectConfig, u32)> {
    let mut value = exact_json(bytes)?;
    let object = value.as_object_mut().ok_or_else(|| error("Profil ou configuration JSON invalide."))?;
    let version = match object.get("version") {
        None => 1,
        Some(v) => v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| error("Version de profil invalide."))?,
    };
    if !matches!(version, 1 | 2) { return Err(error("Version de profil non prise en charge.")); }
    if let Some(fields) = object.get_mut("fields").and_then(Value::as_array_mut) {
        for field in fields {
            let field = field.as_object_mut().ok_or_else(|| error("Champ du dictionnaire invalide."))?;
            for key in ["minimum", "maximum"] {
                if let Some(v) = field.get_mut(key) {
                    if let Value::Number(raw) = v { *v = Value::String(raw.to_string()); }
                }
            }
        }
    }
    if let Some(chronology) = object.get_mut("chronology").and_then(Value::as_array_mut) {
        for rule in chronology {
            if let Some(rule) = rule.as_object_mut() { rule.entry("enabled").or_insert(Value::Bool(true)); }
        }
    }
    let mut config: ProjectConfig = serde_json::from_value(value)?;
    config.version = 2;
    if config.header_row == 0 || config.header_row > 1_048_576 { return Err(error("Ligne d'en-tete invalide.")); }
    Ok((config, version))
}

fn legacy_cell(value: Value, kind: Option<&str>) -> Result<CellValue> {
    let cell = match value {
        Value::Number(raw) => {
            let raw = raw.to_string();
            let kind = if raw.contains(['.', 'e', 'E']) { "decimal" } else { "integer" };
            CellValue::typed(kind, raw)
        },
        Value::Null => CellValue::empty(),
        Value::Bool(v) => CellValue::typed("boolean", v.to_string()),
        Value::String(v) if kind == Some("decimal") => CellValue::typed("decimal", v),
        Value::String(v) if kind == Some("date") => {
            let date = if v.len() == 10 { v } else {
                let date = NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S%.f").map_err(|_| error("Date historique invalide."))?;
                if date.time() != chrono::NaiveTime::from_hms_opt(0, 0, 0).ok_or_else(|| error("Heure invalide."))? {
                    return Err(error("Une decision historique contient une heure non nulle. Conversion en date refusee pour eviter une perte."));
                }
                date.date().format("%Y-%m-%d").to_string()
            };
            CellValue::typed("date", date)
        },
        Value::String(v) => CellValue::text(v),
        Value::Object(v) if v.contains_key("kind") && v.contains_key("value") => serde_json::from_value(Value::Object(v))?,
        _ => return Err(error("Valeur historique non prise en charge.")),
    };
    validate_cell(&cell)?;
    Ok(cell)
}

fn decisions_from_json(bytes: &[u8], version: u32) -> Result<Decisions> {
    let value = exact_json(bytes)?;
    let object = value.as_object().ok_or_else(|| error("Decisions de projet invalides."))?;
    let mut result = BTreeMap::new();
    for (key, value) in object {
        let mut value = value.as_object().cloned().ok_or_else(|| error("Decision invalide."))?;
        if version == 1 {
            let kind = value.remove("value_kind");
            let cell = legacy_cell(value.remove("value").unwrap_or(Value::Null), kind.as_ref().and_then(Value::as_str))?;
            value.insert("value".into(), serde_json::to_value(cell)?);
        }
        let decision: Decision = serde_json::from_value(Value::Object(value))?;
        validate_decision(key, &decision)?;
        result.insert(key.clone(), decision);
    }
    Ok(result)
}

fn validate_decision(key: &str, decision: &Decision) -> Result<()> {
    let (sheet, coordinates) = key.rsplit_once('!').ok_or_else(|| error("Cle de decision invalide."))?;
    let (row, col) = coordinates.split_once(':').ok_or_else(|| error("Cle de decision invalide."))?;
    let row: u32 = row.parse().map_err(|_| error("Ligne de decision invalide."))?;
    let col: u32 = col.parse().map_err(|_| error("Colonne de decision invalide."))?;
    if sheet.is_empty() || row == 0 || row > 1_048_576 || col == 0 || col > 16_384 || cell_key(sheet, row, col) != key {
        return Err(error("Coordonnees de decision invalides."));
    }
    if !matches!(decision.action.as_str(), "keep" | "set" | "clear") { return Err(error("Action de decision inconnue.")); }
    validate_cell(&decision.value)?;
    if decision.action == "set" && matches!(decision.value.kind.as_str(), "formula" | "error") {
        return Err(error("Une decision ne peut pas introduire de formule ou d'erreur Excel."));
    }
    Ok(())
}

fn destination_parent(path: &Path) -> Result<&Path> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() { return Err(error("Le dossier de destination n'existe pas.")); }
    Ok(parent)
}

fn publish(temp: tempfile::NamedTempFile, path: &Path, previous: Option<String>) -> Result<()> {
    match previous {
        Some(expected) => {
            if sha256(path)? != expected { return Err(error("Le fichier de destination a change pendant l'enregistrement.")); }
            temp.persist(path).map_err(|e| io_error("Enregistrement impossible (fichier verrouille ?)", e.error))?;
        },
        None => { temp.persist_noclobber(path).map_err(|e| io_error("Le fichier de destination existe deja ou ne peut pas etre cree", e.error))?; },
    }
    Ok(())
}

pub fn save_profile(path: &Path, config: &ProjectConfig) -> Result<()> {
    extension(path, "json")?;
    protect_paths(path, &[&config.source_path, &config.original_path, &config.dictionary_path])?;
    let parent = destination_parent(path)?;
    let previous = if path.exists() {
        let bytes = read_bounded(path, MAX_JSON, &JobControl::default())?;
        let (_, version) = config_from_json(&bytes)?;
        if version == 1 { return Err(error("Un profil historique ne peut pas etre ecrase. Choisissez un nouveau nom pour la version 2.")); }
        Some(digest(&bytes))
    } else { None };
    let mut saved = config.clone();
    saved.version = 2;
    saved.source_path.clear(); saved.original_path.clear(); saved.dictionary_path.clear(); saved.sheet.clear();
    let bytes = serde_json::to_vec_pretty(&saved)?;
    config_from_json(&bytes)?;
    let mut temp = tempfile::Builder::new().prefix(".fdx-profile-").suffix(".json").tempfile_in(parent)?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    let reopened = read_bounded(temp.path(), MAX_JSON, &JobControl::default())?;
    if reopened != bytes { return Err(error("Le profil temporaire n'a pas pu etre verifie.")); }
    config_from_json(&reopened)?;
    protect_paths(path, &[&config.source_path, &config.original_path, &config.dictionary_path])?;
    publish(temp, path, previous)
}

pub fn load_profile(path: &Path, current: &ProjectConfig) -> Result<ProjectConfig> {
    extension(path, "json")?;
    let bytes = read_bounded(path, MAX_JSON, &JobControl::default())?;
    let (mut loaded, _) = config_from_json(&bytes)?;
    loaded.source_path = current.source_path.clone();
    loaded.original_path = current.original_path.clone();
    loaded.dictionary_path = current.dictionary_path.clone();
    loaded.sheet = current.sheet.clone();
    for mapping in &mut loaded.mappings { mapping.confirmed = false; }
    Ok(loaded)
}

fn db_error(e: rusqlite::Error) -> FdxError { io_error("Projet SQLite invalide ou inaccessible", e) }

fn read_database(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    if fs::metadata(path)?.len() > MAX_DATABASE { return Err(error("Projet SQLite trop volumineux.")); }
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(db_error)?;
    db.pragma_update(None, "trusted_schema", false).map_err(db_error)?;
    db.execute_batch("BEGIN").map_err(db_error)?;
    let kind: String = db.query_row("SELECT type FROM sqlite_schema WHERE name = 'project'", [], |row| row.get(0)).map_err(db_error)?;
    if kind != "table" { return Err(error("La structure du projet SQLite n'est pas une table.")); }
    let check: String = db.query_row("PRAGMA quick_check(1)", [], |row| row.get(0)).map_err(db_error)?;
    if check != "ok" { return Err(error("Base de projet endommagee.")); }
    let mut statement = db.prepare("SELECT key, length(value), typeof(value) FROM project LIMIT 33").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    let mut keys = Vec::new();
    let mut total = 0u64;
    while let Some(row) = rows.next().map_err(db_error)? {
        let key: String = row.get(0).map_err(db_error)?;
        let length: i64 = row.get(1).map_err(db_error)?;
        let kind: String = row.get(2).map_err(db_error)?;
        let limit = if key == "source" { MAX_FILE } else { MAX_JSON };
        if length < 0 || length as u64 > limit || !matches!(kind.as_str(), "blob" | "text") || key.len() > 128 {
            return Err(error("Donnees de projet invalides ou trop volumineuses."));
        }
        total += length as u64;
        if total > MAX_DATABASE || keys.len() >= 32 || keys.contains(&key) { return Err(error("Trop de donnees ou cles dupliquees dans le projet.")); }
        keys.push(key);
    }
    drop(rows); drop(statement);
    let mut result = BTreeMap::new();
    for key in keys {
        let value = db.query_row("SELECT value FROM project WHERE key = ?1", [&key], |row| {
            match row.get_ref(0)? {
                ValueRef::Blob(v) | ValueRef::Text(v) => Ok(v.to_vec()),
                _ => Err(rusqlite::Error::InvalidQuery),
            }
        }).map_err(db_error)?;
        result.insert(key, value);
    }
    db.execute_batch("COMMIT").map_err(db_error)?;
    Ok(result)
}

fn required<'a>(data: &'a BTreeMap<String, Vec<u8>>, key: &str) -> Result<&'a [u8]> {
    data.get(key).map(Vec::as_slice).ok_or_else(|| error(format!("Donnee manquante dans le projet : {key}.")))
}

fn database_version(data: &BTreeMap<String, Vec<u8>>) -> Result<u32> {
    match required(data, "version")? { b"1" => Ok(1), b"2" => Ok(2), _ => Err(error("Version de projet non prise en charge.")) }
}

fn decode_project(data: &BTreeMap<String, Vec<u8>>) -> Result<SavedProject> {
    let version = database_version(data)?;
    let source = required(data, "source")?;
    if digest(source).as_bytes() != required(data, "source_hash")? { return Err(error("La copie source enregistree dans le projet est endommagee.")); }
    if version == 2 {
        let dictionary = required(data, "dictionary")?;
        if digest(dictionary).as_bytes() != required(data, "dictionary_hash")? { return Err(error("La copie du dictionnaire est endommagee.")); }
    }
    let (config, config_version) = config_from_json(required(data, "config")?)?;
    if config_version != version { return Err(error("Versions de projet et de configuration incoherentes.")); }
    let decisions = decisions_from_json(data.get("decisions").map_or(b"{}".as_slice(), Vec::as_slice), version)?;
    let history = serde_json::from_value(exact_json(data.get("history").map_or(b"[]".as_slice(), Vec::as_slice))?)?;
    let ui_state = exact_json(data.get("ui_state").map_or(b"{}".as_slice(), Vec::as_slice))?;
    Ok(SavedProject { config, decisions, history, ui_state })
}

pub fn save_project(path: &Path, project: &SavedProject) -> Result<()> {
    extension(path, "fdxproj")?;
    let config = &project.config;
    protect_paths(path, &[&config.source_path, &config.original_path, &config.dictionary_path])?;
    let parent = destination_parent(path)?;
    let previous = if path.exists() {
        let hash = sha256(path)?;
        let existing = read_database(path)?;
        if database_version(&existing)? == 1 {
            return Err(error("Le projet historique reste intact. Choisissez un nouveau fichier pour enregistrer la version 2."));
        }
        decode_project(&existing)?;
        if sha256(path)? != hash { return Err(error("Le projet a change pendant sa lecture.")); }
        Some(hash)
    } else { None };
    for (key, decision) in &project.decisions { validate_decision(key, decision)?; }
    let job = JobControl::default();
    let source = read_bounded(Path::new(&config.source_path), MAX_FILE, &job)?;
    let source_hash = digest(&source);
    let package = Package::from_bytes(&source, &job)?;
    data_from_package(&package, source_hash.clone(), &job)?;
    umya_read(&source, &job)?;
    let dictionary = if config.dictionary_path.is_empty() { vec![] } else { read_bounded(Path::new(&config.dictionary_path), MAX_JSON, &job)? };
    let dictionary_hash = digest(&dictionary);
    let mut saved = config.clone(); saved.version = 2;
    let mut data = BTreeMap::new();
    data.insert("version".to_owned(), b"2".to_vec());
    data.insert("config".to_owned(), serde_json::to_vec(&saved)?);
    data.insert("decisions".to_owned(), serde_json::to_vec(&project.decisions)?);
    data.insert("history".to_owned(), serde_json::to_vec(&project.history)?);
    data.insert("ui_state".to_owned(), serde_json::to_vec(&project.ui_state)?);
    data.insert("source".to_owned(), source);
    data.insert("source_hash".to_owned(), source_hash.as_bytes().to_vec());
    data.insert("dictionary".to_owned(), dictionary);
    data.insert("dictionary_hash".to_owned(), dictionary_hash.as_bytes().to_vec());
    decode_project(&data)?;
    let temp = tempfile::Builder::new().prefix(".fdx-project-").suffix(".fdxproj").tempfile_in(parent)?;
    {
        let mut db = Connection::open(temp.path()).map_err(db_error)?;
        db.pragma_update(None, "journal_mode", "DELETE").map_err(db_error)?;
        db.pragma_update(None, "synchronous", "FULL").map_err(db_error)?;
        let tx = db.transaction().map_err(db_error)?;
        tx.execute("CREATE TABLE project (key TEXT PRIMARY KEY, value BLOB NOT NULL)", []).map_err(db_error)?;
        for (key, value) in &data { tx.execute("INSERT INTO project (key, value) VALUES (?1, ?2)", params![key, value]).map_err(db_error)?; }
        tx.commit().map_err(db_error)?;
        db.close().map_err(|(_, e)| db_error(e))?;
    }
    temp.as_file().sync_all()?;
    let reopened = read_database(temp.path())?;
    if reopened != data { return Err(error("Le projet temporaire n'a pas pu etre verifie.")); }
    decode_project(&reopened)?;
    if sha256(Path::new(&config.source_path))? != source_hash || (!config.dictionary_path.is_empty() && sha256(Path::new(&config.dictionary_path))? != dictionary_hash) {
        return Err(error("La source ou le dictionnaire a change pendant l'enregistrement."));
    }
    protect_paths(path, &[&config.source_path, &config.original_path, &config.dictionary_path])?;
    publish(temp, path, previous)
}

fn write_copy(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if sha256(path)? != digest(bytes) { return Err(error("La copie extraite du projet n'a pas pu etre verifiee.")); }
    Ok(())
}

pub fn open_project(path: &Path, cache_directory: &Path) -> Result<SavedProject> {
    extension(path, "fdxproj")?;
    let project_hash = sha256(path)?;
    let data = read_database(path)?;
    let mut project = decode_project(&data)?;
    let source = required(&data, "source")?;
    let job = JobControl::default();
    let package = Package::from_bytes(source, &job)?;
    data_from_package(&package, digest(source), &job)?;
    umya_read(source, &job)?;
    if sha256(path)? != project_hash { return Err(error("Le projet a change pendant l'ouverture.")); }
    fs::create_dir_all(cache_directory)?;
    let folder = tempfile::Builder::new().prefix("fdx-session-").tempdir_in(cache_directory)?;
    let source_path = folder.path().join("source.xlsx");
    write_copy(&source_path, source)?;
    if project.config.original_path.is_empty() { project.config.original_path = project.config.source_path.clone(); }
    project.config.source_path = source_path.to_string_lossy().into_owned();
    project.config.dictionary_path.clear();
    if let Some(dictionary) = data.get("dictionary").filter(|v| !v.is_empty()) {
        let dictionary_path = folder.path().join("dictionnaire.txt");
        write_copy(&dictionary_path, dictionary)?;
        project.config.dictionary_path = dictionary_path.to_string_lossy().into_owned();
    }
    // The caller owns cache lifecycle. Retain only after every validation and write
    // succeeds; failures automatically remove the entire temporary session directory.
    let _ = folder.keep();
    Ok(project)
}
