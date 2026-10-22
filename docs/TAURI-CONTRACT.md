# Tauri implementation contract

All paths relative to repository. All builds/tests requiring dependencies run on GitHub Actions only. Never install dependencies or build locally. Original Python sources/tests remain a reference, not shipped.

## Core Rust APIs (fdx_core)

Models are defined in `crates/fdx-core/src/models.rs`. CellValue is `{kind,value}` with both strings; kinds: empty, text, integer, decimal, date (ISO YYYY-MM-DD), boolean (true/false), formula, error. Numeric values MUST NOT pass through JavaScript Number. Source numeric kind may be decimal even for an integer. Preserve numeric semantics in export comparison.

- dictionary::parse_dictionary(text: &str) -> DictionaryImport
- dictionary::suggest_mappings(headers: &[CellValue], fields: &[FieldSpec]) -> Vec<ColumnMapping>
- engine::validate_config(config: &ProjectConfig) -> Result<()>
- engine::config_hash(config: &ProjectConfig) -> Result<String> (exclude harmonized/report_collision/reports)
- engine::decisions_hash(decisions: &Decisions) -> Result<String>
- engine::analyze(config: &ProjectConfig, source: &WorkbookData, decisions: &Decisions, job: &JobControl) -> Result<AnalysisResult>
- workbooks::sha256(path: &Path) -> Result<String>
- workbooks::read_source(path: &Path, job: &JobControl) -> Result<WorkbookData>
- workbooks::inspect(path: &Path, header_row: u32, job: &JobControl) -> Result<WorkbookInfo>
- exporter::export_excel(config: &ProjectConfig, result: &AnalysisResult, destination: &Path, allow_issues: bool, overwrite: bool, job: &JobControl) -> Result<PathBuf>
- storage::save_profile(path: &Path, config: &ProjectConfig) -> Result<()>
- storage::load_profile(path: &Path, current: &ProjectConfig) -> Result<ProjectConfig>
- storage::save_project(path: &Path, project: &SavedProject) -> Result<()>
- storage::open_project(path: &Path, cache_directory: &Path) -> Result<SavedProject>

Core tests independent of Tauri. No source modification, implicit destructive correction, formula replacement or stale export. Same cell key sheet!row:column (1 based). Core I/O agent must NOT edit models/engine/dictionary/transforms. Engine agent owns these four files and can add methods to models without breaking contract.

## Frontend bridge

`invoke('fdx', {request: {op, session_id?, task_id, revision?, payload}, onEvent: Channel<Progress>})` returns JSON. `cancel_task({taskId})` is separate. Desktop uses @tauri-apps/api/core; browser uses explicit fixture adapter (NO real-file processing/export). No broad filesystem/shell permission. Native dialog plugin selects files; returned paths sent to bridge. Op payloads and responses:

- create: {name,family} -> Session
- demo: {} -> Session (native generates synthetic XLSX and dictionary in app cache)
- recent: {} -> array of {id,name,family,updated,path,status}; saved project catalog only
- inspect: {path,header_row} -> WorkbookInfo (does not mutate session)
- import: {path,dictionary_path?,sheet?,header_row} -> Session (sets source, reads dict and suggests mappings, clears decisions/results)
- dictionary: {path? ,text?} -> Session (parsed fields + notes, mappings proposed again)
- configure: {config: ProjectConfig} -> Session (keep safe source paths from session; validates on analyze, invalidates result)
- analyze: {} -> Session (result summary; full results kept backend)
- results: {view: 'findings'|'changes'|'history',offset,limit,query?,kind?,sort?:'row'|'kind',descending?} -> {items,total}; findings always include original coordinates
- decide: {decisions: Record<cellKey,Decision>} -> Session (accumulate; snapshot undo/redo; no analysis)
- undo/redo: {} -> Session (decision snapshots; invalidates result)
- profile_load: {path} -> Session (mapping confirmation reset)
- profile_save: {path} -> {path}
- project_open: {path} -> Session (legacy import, discard cached analysis)
- project_save: {path,ui_state?} -> {path} (original legacy project not silently overwritten)
- export: {path,allow_issues,overwrite} -> {path} (must have current result/decisions; replace only on explicit consent)
- open_path: {path,folder?:boolean} -> null (only known successful export/saved project; do not accept arbitrary command)

Session = {id,name,family,revision,config,workbook: WorkbookInfo|null,notes:string[],summary: {row_count,column_count,findings,changes,counts:Record<string,number>}|null,dirty:boolean,pending_decisions:number,can_undo:boolean,can_redo:boolean,export_path:string|null,project_path:string|null}.

Every mutating request includes expected revision. Reject mismatches, including stale task completion. Error is {code,message,details?}, no stack trace in primary UI. task_id UUID generated frontend. User-visible errors French. Progress task_id/phase/current/total(message total null during indeterminate work)/message. Prefer full-path only in native file selection and detailed errors; never transmit private data to Pages.

Reports codes control/original/verify; harmonized false means no reports. Configuration version 2, v1 imported with defaults. UI mode guided/advanced shares same config and decisions. Types mirror models.rs. Fields minimum/maximum decimal strings or null. Defaults defined there.
