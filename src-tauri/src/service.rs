use fdx_core::{demo, dictionary, engine, exporter, models::*, storage, workbooks};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::{BTreeMap, HashMap}, fs, path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::AtomicBool}};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct Request {
    pub op: String,
    #[serde(default)] pub session_id: Option<String>,
    pub task_id: String,
    #[serde(default)] pub revision: Option<u64>,
    #[serde(default)] pub payload: Value,
}

#[derive(Clone)]
struct Session {
    id: String, name: String, family: String, revision: u64,
    config: ProjectConfig, workbook: Option<WorkbookInfo>, source: Option<Arc<WorkbookData>>,
    notes: Vec<String>, result: Option<Arc<AnalysisResult>>, decisions: Decisions,
    history: Vec<Value>, undo: Vec<Decisions>, redo: Vec<Decisions>, dirty: bool,
    export_path: Option<String>, project_path: Option<String>, ui_state: Value,
}
impl Session {
    fn new(name: String, family: String) -> Self {
        Self { id: Uuid::new_v4().to_string(), name, family, revision: 0, config: ProjectConfig::default(), workbook: None, source: None, notes: vec![], result: None, decisions: Decisions::new(), history: vec![], undo: vec![], redo: vec![], dirty: true, export_path: None, project_path: None, ui_state: Value::Null }
    }
    fn view(&self) -> Value {
        let summary = self.result.as_ref().map(|r| {
            let mut counts = BTreeMap::<String, usize>::new();
            for finding in &r.findings { *counts.entry(finding.kind.clone()).or_default() += 1; }
            json!({"row_count":r.row_count,"column_count":r.column_count,"findings":r.findings.len(),"changes":r.changes.len(),"counts":counts})
        });
        json!({"id":self.id,"name":self.name,"family":self.family,"revision":self.revision,"config":self.config,"workbook":self.workbook,"notes":self.notes,"summary":summary,"dirty":self.dirty,"pending_decisions":if self.dirty {self.decisions.len()} else {0},"can_undo":!self.undo.is_empty(),"can_redo":!self.redo.is_empty(),"export_path":self.export_path,"project_path":self.project_path,"ui_state":self.ui_state})
    }
    fn invalidate(&mut self) { self.dirty = true; self.export_path = None; }
    fn proposed_mappings(&mut self) {
        if let Some(info) = &self.workbook {
            if let Some(sheet) = info.sheets.iter().find(|s| s.name == self.config.sheet) {
                self.config.mappings = dictionary::suggest_mappings(&sheet.headers, &self.config.fields);
            }
        }
    }
    fn read_sources(&mut self, job: &JobControl) -> Result<()> {
        let path = Path::new(&self.config.source_path);
        self.workbook = Some(workbooks::inspect(path, self.config.header_row, job)?);
        self.source = Some(Arc::new(workbooks::read_source(path, job)?));
        Ok(())
    }
    fn record(&mut self, action: &str, value: Value) {
        self.history.push(json!({"at":chrono::Utc::now().to_rfc3339(),"action":action,"details":value}));
    }
}

pub struct AppState {
    root: PathBuf,
    sessions: Mutex<HashMap<String, Arc<Mutex<Session>>>>,
    pub tasks: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value.get(key).and_then(Value::as_str).filter(|v| !v.trim().is_empty()).ok_or_else(|| error(format!("Information manquante : {key}")))
}
fn flag(value: &Value, key: &str) -> bool { value.get(key).and_then(Value::as_bool).unwrap_or(false) }
fn header(value: &Value) -> Result<u32> {
    let row = value.get("header_row").and_then(Value::as_u64).unwrap_or(1);
    if !(1..=1_048_576).contains(&row) { return Err(error("La ligne d'en-tete doit correspondre a une ligne Excel.")); }
    Ok(row as u32)
}
fn dictionary_text(path: &Path) -> Result<String> {
    if !matches!(path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(), "md"|"txt") { return Err(error("Choisissez un dictionnaire .md ou .txt.")); }
    if fs::metadata(path)?.len() > 4_000_000 { return Err(error("Le dictionnaire depasse la limite de 4 Mo.")); }
    let bytes = fs::read(path)?;
    if bytes.starts_with(&[0xff,0xfe]) || bytes.starts_with(&[0xfe,0xff]) {
        if bytes.len() % 2 != 0 { return Err(error("Le dictionnaire UTF-16 est incomplet.")); }
        let little = bytes[0] == 0xff;
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|b| if little { u16::from_le_bytes([b[0],b[1]]) } else { u16::from_be_bytes([b[0],b[1]]) }).collect();
        return String::from_utf16(&units).map_err(|_| error("Encodage du dictionnaire invalide."));
    }
    match std::str::from_utf8(&bytes) {
        Ok(value) => Ok(value.trim_start_matches('\u{feff}').to_owned()),
        Err(_) => Ok(encoding_rs::WINDOWS_1252.decode(&bytes).0.into_owned()),
    }
}

impl AppState {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("cache"))?;
        let state = Self { root, sessions: Mutex::new(HashMap::new()), tasks: Mutex::new(HashMap::new()) };
        state.catalog()?;
        Ok(state)
    }
    fn catalog(&self) -> Result<rusqlite::Connection> {
        let db = rusqlite::Connection::open(self.root.join("catalog.sqlite")).map_err(|e| error(e.to_string()))?;
        db.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e| error(e.to_string()))?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS recent (path TEXT PRIMARY KEY, id TEXT NOT NULL, name TEXT NOT NULL, family TEXT NOT NULL, updated TEXT NOT NULL, status TEXT NOT NULL)").map_err(|e| error(e.to_string()))?;
        Ok(db)
    }
    fn register(&self, session: Session) -> Result<Value> {
        let result = session.view();
        self.sessions.lock().map_err(|_| error("Etat indisponible."))?.insert(session.id.clone(), Arc::new(Mutex::new(session)));
        Ok(result)
    }
    pub fn execute(&self, request: Request, job: &JobControl) -> Result<Value> {
        job.check()?;
        if request.task_id.len() > 80 || Uuid::parse_str(&request.task_id).is_err() { return Err(error("Identifiant de traitement invalide.")); }
        let p = &request.payload;
        match request.op.as_str() {
            "create" => return self.register(Session::new(text(p,"name")?.chars().take(200).collect(), text(p,"family")?.chars().take(100).collect())),
            "demo" => {
                let mut session = Session::new("Exemple fictif".into(), "Marches signes".into());
                let folder = self.root.join("cache").join(&session.id);
                let (source, dict) = demo::create(&folder, 3)?;
                session.config.source_path = source.to_string_lossy().into_owned();
                session.config.original_path = session.config.source_path.clone();
                session.config.dictionary_path = dict.to_string_lossy().into_owned();
                session.read_sources(job)?;
                session.config.sheet = session.workbook.as_ref().unwrap().suggested_sheet.clone();
                let parsed = dictionary::parse_dictionary(&dictionary_text(&dict)?);
                session.config.fields = parsed.fields; session.notes = parsed.notes; session.proposed_mappings();
                return self.register(session);
            }
            "recent" => {
                let db = self.catalog()?;
                let mut statement = db.prepare("SELECT id,name,family,updated,path,status FROM recent ORDER BY updated DESC LIMIT 50").map_err(|e| error(e.to_string()))?;
                let rows = statement.query_map([], |row| Ok(json!({"id":row.get::<_,String>(0)?,"name":row.get::<_,String>(1)?,"family":row.get::<_,String>(2)?,"updated":row.get::<_,String>(3)?,"path":row.get::<_,String>(4)?,"status":row.get::<_,String>(5)?}))).map_err(|e| error(e.to_string()))?;
                let mut values = vec![];
                for row in rows { values.push(row.map_err(|e| error(e.to_string()))?); }
                return Ok(json!(values));
            }
            "inspect" => return Ok(serde_json::to_value(workbooks::inspect(Path::new(text(p,"path")?),header(p)?,job)?)?),
            "project_open" => {
                let path = Path::new(text(p,"path")?);
                let saved = storage::open_project(path, &self.root.join("cache"))?;
                let name = saved.ui_state.get("name").and_then(Value::as_str).unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("Projet"));
                let family = saved.ui_state.get("family").and_then(Value::as_str).unwrap_or("Marches signes");
                let mut session = Session::new(name.into(),family.into());
                session.config = saved.config; session.decisions = saved.decisions; session.history = saved.history; session.ui_state = saved.ui_state;
                session.read_sources(job)?; session.project_path = Some(path.to_string_lossy().into_owned());
                session.notes.push("Projet ouvert : verifiez les parametres puis relancez l'analyse.".into());
                return self.register(session);
            }
            _ => {}
        }
        let session_id = request.session_id.as_ref().ok_or_else(|| error("Ouvrez d'abord un projet."))?;
        let session = self.sessions.lock().map_err(|_| error("Etat indisponible."))?.get(session_id).cloned().ok_or_else(|| error("Projet ferme ou introuvable."))?;
        let mut guard = session.lock().map_err(|_| error("Projet indisponible."))?;
        // Commands operate on a candidate: failed/cancelled operations never replace session state.
        let mut s = guard.clone();
        if request.op != "results" && request.revision != Some(s.revision) { return Err(error("Les parametres ont change. Reprenez l'operation depuis l'etat actuel.")); }
        let response = match request.op.as_str() {
            "import" => {
                let path = text(p,"path")?;
                let mut config = ProjectConfig { source_path: path.into(), original_path: path.into(), header_row: header(p)?, ..ProjectConfig::default() };
                let info = workbooks::inspect(Path::new(path),config.header_row,job)?;
                config.sheet = p.get("sheet").and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or(&info.suggested_sheet).into();
                let sheet = info.sheets.iter().find(|sheet| sheet.name == config.sheet).ok_or_else(|| error("Cette feuille n'existe pas."))?;
                if config.header_row > sheet.rows { return Err(error("La ligne d'en-tete depasse les donnees de cette feuille.")); }
                s.notes.clear();
                if let Some(dict) = p.get("dictionary_path").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                    let parsed = dictionary::parse_dictionary(&dictionary_text(Path::new(dict))?);
                    config.dictionary_path = dict.into(); config.fields = parsed.fields; s.notes = parsed.notes;
                }
                s.config = config; s.workbook = Some(info); s.source = Some(Arc::new(workbooks::read_source(Path::new(path),job)?));
                s.proposed_mappings(); s.decisions.clear(); s.undo.clear(); s.redo.clear(); s.result = None; s.invalidate();
                s.record("import",json!({"sheet":s.config.sheet})); Value::Null
            }
            "dictionary" => {
                let content = if let Some(path) = p.get("path").and_then(Value::as_str) { let content = dictionary_text(Path::new(path))?; s.config.dictionary_path = path.into(); content } else { text(p,"text")?.to_owned() };
                if content.len() > 4_000_000 { return Err(error("Dictionnaire trop volumineux.")); }
                let parsed = dictionary::parse_dictionary(&content);
                s.config.fields = parsed.fields; s.notes = parsed.notes; s.proposed_mappings(); s.invalidate(); Value::Null
            }
            "configure" => {
                let mut config: ProjectConfig = serde_json::from_value(p.get("config").cloned().ok_or_else(|| error("Parametres manquants."))?)?;
                config.source_path = s.config.source_path.clone(); config.original_path = s.config.original_path.clone(); config.dictionary_path = s.config.dictionary_path.clone();
                let source_selection_changed = config.sheet != s.config.sheet || config.header_row != s.config.header_row;
                let changed = engine::config_hash(&config)? != engine::config_hash(&s.config)?;
                if source_selection_changed {
                    s.workbook = Some(workbooks::inspect(Path::new(&config.source_path),config.header_row,job)?);
                    let sheet = s.workbook.as_ref().unwrap().sheets.iter().find(|v| v.name == config.sheet).ok_or_else(|| error("Feuille introuvable."))?;
                    if config.header_row == 0 || config.header_row > sheet.rows { return Err(error("Ligne d'en-tete hors des donnees.")); }
                    s.decisions.clear(); s.undo.clear(); s.redo.clear(); s.result = None;
                    for mapping in &mut config.mappings { mapping.confirmed = false; }
                }
                s.config = config;
                if changed { s.invalidate(); } else { s.export_path = None; }
                Value::Null
            }
            "analyze" => {
                engine::validate_config(&s.config)?;
                let source = s.source.as_ref().ok_or_else(|| error("Importez un classeur."))?;
                if workbooks::sha256(Path::new(&s.config.source_path))? != source.source_hash { return Err(error("Le fichier source a change. Importez-le a nouveau avant de continuer.")); }
                let result = engine::analyze(&s.config,source,&s.decisions,job)?;
                job.check()?;
                if workbooks::sha256(Path::new(&s.config.source_path))? != result.source_hash { return Err(error("La source a change pendant l'analyse. Aucun resultat n'est conserve.")); }
                s.record("analyze",json!({"rows":result.row_count,"issues":result.findings.len(),"changes":result.changes.len()}));
                s.result = Some(Arc::new(result)); s.dirty = false; s.export_path = None; Value::Null
            }
            "results" => return results(&s,p),
            "decide" => {
                let values: Decisions = serde_json::from_value(p.get("decisions").cloned().ok_or_else(|| error("Decisions manquantes."))?)?;
                let result = s.result.as_ref().ok_or_else(|| error("Analysez le fichier avant de prendre des decisions."))?;
                for (key, decision) in &values {
                    if !result.findings.iter().any(|f| &f.key == key) && !result.changes.iter().any(|c| &c.key == key) { return Err(error("La cellule ne fait pas partie du resultat analyse.")); }
                    if !matches!(decision.action.as_str(),"keep"|"set"|"clear") { return Err(error("Decision inconnue.")); }
                    if decision.value.kind == "formula" { return Err(error("Une correction ne peut pas introduire une formule.")); }
                }
                s.undo.push(s.decisions.clone()); if s.undo.len() > 100 { s.undo.remove(0); } s.redo.clear();
                s.record("decide",serde_json::to_value(&values)?); s.decisions.extend(values); s.invalidate(); Value::Null
            }
            "undo" => {
                if let Some(previous) = s.undo.pop() { s.redo.push(s.decisions.clone()); s.decisions = previous; s.invalidate(); s.record("undo",Value::Null); } Value::Null
            }
            "redo" => {
                if let Some(next) = s.redo.pop() { s.undo.push(s.decisions.clone()); s.decisions = next; s.invalidate(); s.record("redo",Value::Null); } Value::Null
            }
            "profile_load" => { s.config = storage::load_profile(Path::new(text(p,"path")?),&s.config)?; s.invalidate(); Value::Null }
            "profile_save" => { let path = text(p,"path")?; storage::save_profile(Path::new(path),&s.config)?; return Ok(json!({"path":path})); }
            "project_save" => {
                let path = text(p,"path")?;
                let mut ui = p.get("ui_state").cloned().unwrap_or_else(|| s.ui_state.clone());
                if !ui.is_object() { ui = json!({}); }
                ui["name"] = json!(s.name); ui["family"] = json!(s.family);
                storage::save_project(Path::new(path),&SavedProject { config:s.config.clone(),decisions:s.decisions.clone(),history:s.history.clone(),ui_state:ui.clone() })?;
                let db = self.catalog()?;
                db.execute("INSERT OR REPLACE INTO recent(path,id,name,family,updated,status) VALUES(?1,?2,?3,?4,?5,?6)",rusqlite::params![path,s.id,s.name,s.family,chrono::Utc::now().to_rfc3339(),if s.dirty { "A analyser" } else { "Enregistre" }]).map_err(|e| error(e.to_string()))?;
                s.project_path = Some(path.into()); s.ui_state = ui;
                // Saving metadata does not change the frontend configuration revision.
                *guard = s; return Ok(json!({"path":path}));
            }
            "export" => {
                if s.dirty { return Err(error("Verifiez vos corrections avant d'exporter.")); }
                let result = s.result.as_ref().ok_or_else(|| error("Analysez le classeur avant l'export."))?;
                if engine::decisions_hash(&s.decisions)? != result.decisions_hash { return Err(error("Les decisions ont change. Relancez la verification.")); }
                let destination = exporter::export_excel(&s.config,result,Path::new(text(p,"path")?),flag(p,"allow_issues"),flag(p,"overwrite"),job)?;
                let path = destination.to_string_lossy().into_owned(); s.export_path = Some(path.clone()); s.record("export",json!({"path":path}));
                *guard = s; return Ok(json!({"path":path}));
            }
            "open_path" => {
                let path = Path::new(text(p,"path")?);
                let normalized = fs::canonicalize(path)?;
                let known = [&s.export_path,&s.project_path].into_iter().flatten().filter_map(|v| fs::canonicalize(v).ok()).any(|v| v == normalized);
                if !known { return Err(error("Ce fichier n'a pas ete cree par cette session.")); }
                let target = if flag(p,"folder") { normalized.parent().ok_or_else(|| error("Dossier introuvable."))?.to_path_buf() } else { normalized };
                #[cfg(target_os="windows")]
                { use std::os::windows::process::CommandExt; std::process::Command::new("explorer.exe").arg(target).creation_flags(0x08000000).spawn()?; }
                #[cfg(not(target_os="windows"))]
                { let _ = target; return Err(error("Ouverture native disponible sous Windows.")); }
                #[allow(unreachable_code)]
                return Ok(Value::Null);
            }
            _ => return Err(error("Operation inconnue.")),
        };
        job.check()?;
        s.revision += 1;
        let response = if response.is_null() { s.view() } else { response };
        *guard = s;
        Ok(response)
    }
}

fn results(session: &Session, p: &Value) -> Result<Value> {
    let offset = p.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let limit = p.get("limit").and_then(Value::as_u64).unwrap_or(100).clamp(1,500) as usize;
    let query = p.get("query").and_then(Value::as_str).unwrap_or("").to_lowercase();
    let kind = p.get("kind").and_then(Value::as_str).unwrap_or("");
    let view = p.get("view").and_then(Value::as_str).unwrap_or("findings");
    let result = session.result.as_ref();
    let items: Vec<Value> = match view {
        "history" => session.history.clone(),
        "changes" => result.map(|r| r.changes.iter().map(|v| serde_json::to_value(v).unwrap_or(Value::Null)).collect()).unwrap_or_default(),
        "findings" => result.map(|r| r.findings.iter().map(|v| serde_json::to_value(v).unwrap_or(Value::Null)).collect()).unwrap_or_default(),
        _ => return Err(error("Vue de resultat inconnue.")),
    };
    let mut items: Vec<Value> = items.into_iter().filter(|v| (kind.is_empty() || v.get("kind").and_then(Value::as_str) == Some(kind)) && (query.is_empty() || v.to_string().to_lowercase().contains(&query))).collect();
    if p.get("sort").and_then(Value::as_str) == Some("kind") { items.sort_by_key(|v| v.get("kind").and_then(Value::as_str).unwrap_or("").to_owned()); }
    else if view != "history" { items.sort_by_key(|v| (v.get("row").and_then(Value::as_u64).unwrap_or(0),v.get("column").and_then(Value::as_u64).unwrap_or(0))); }
    if flag(p,"descending") { items.reverse(); }
    let total = items.len();
    Ok(json!({"items":items.into_iter().skip(offset).take(limit).collect::<Vec<_>>(),"total":total}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(op: &str, session: Option<&Value>, payload: Value) -> Request { Request { op:op.into(),session_id:session.and_then(|s|s["id"].as_str()).map(str::to_owned),task_id:Uuid::new_v4().to_string(),revision:session.and_then(|s|s["revision"].as_u64()),payload } }
    #[test]
    fn native_demo_and_revision_guard() {
        let root = tempfile::tempdir().unwrap(); let app = AppState::new(root.path().to_owned()).unwrap();
        let s = app.execute(request("demo",None,json!({})),&JobControl::default()).unwrap();
        assert!(s["config"]["fields"].as_array().unwrap().len() >= 5);
        let mut config = s["config"].clone();
        for mapping in config["mappings"].as_array_mut().unwrap() { mapping["confirmed"] = json!(true); }
        let next = app.execute(request("configure",Some(&s),json!({"config":config})),&JobControl::default()).unwrap();
        assert!(app.execute(request("analyze",Some(&s),json!({})),&JobControl::default()).is_err());
        let analyzed = app.execute(request("analyze",Some(&next),json!({})),&JobControl::default()).unwrap();
        assert_eq!(analyzed["summary"]["row_count"],3);
        assert_eq!(analyzed["dirty"],false);
    }
}
