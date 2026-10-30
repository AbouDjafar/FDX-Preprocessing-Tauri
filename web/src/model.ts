// Wire names and defaults mirror crates/fdx-core/src/models.rs (configuration v2).
export interface CellValue { kind: string; value: string }
export interface FieldSpec {
  code: string; label: string; dtype: string; role: string; max_length: number | null;
  date_format: string; required: boolean; pattern: string; allowed_values: string[];
  minimum: string | null; maximum: string | null;
}
export interface ColumnMapping { column: number; field_code: string | null; confirmed: boolean; score: number }
export interface FieldRule {
  normalize_spaces: boolean; convert: boolean; decimal_separator: string; thousands_separator: string;
  currency_tokens: string[]; date_order: string; phone_region: string; phone_policy: string;
  phone_separator: string; length_policy: string; invalid_policy: string; replacements: Record<string, string>;
}
export interface ChronologyRule { before: string; after: string; enabled: boolean }
export interface SegmentationRule {
  enabled: boolean; source_column: number; name_column: number | null; bp_column: number | null;
  phone_column: number | null; location_column: number | null; overwrite: boolean;
}
export interface ProjectConfig {
  version: number; source_path: string; original_path: string; dictionary_path: string; sheet: string;
  header_row: number; fields: FieldSpec[]; mappings: ColumnMapping[]; rules: Record<string, FieldRule>;
  chronology: ChronologyRule[]; segmentation: SegmentationRule; harmonized: boolean;
  report_collision: string; reports: string[];
}
export interface Decision { action: string; value: CellValue; allow_multiple: boolean; reason: string }
export type Decisions = Record<string, Decision>;
export interface Finding {
  key: string; row: number; column: number; field_code: string; kind: string; message: string;
  original: CellValue; current: CellValue; proposed: CellValue | null; choices: CellValue[];
}
export interface CellChange {
  key: string; row: number; column: number; field_code: string; before: CellValue; after: CellValue; reason: string;
}
export type HistoryEntry = Record<string, unknown>;
export interface SheetInfo { name: string; rows: number; columns: number; headers: CellValue[]; preview: CellValue[][]; suggested_header: number }
export interface WorkbookInfo { path: string; sheets: SheetInfo[]; suggested_sheet: string }
export interface Summary { row_count: number; column_count: number; findings: number; changes: number; counts: Record<string, number> }
export interface Session {
  id: string; name: string; family: string; revision: number; config: ProjectConfig;
  workbook: WorkbookInfo | null; notes: string[]; summary: Summary | null; dirty: boolean;
  pending_decisions: number; can_undo: boolean; can_redo: boolean; export_path: string | null; project_path: string | null; ui_state?: unknown;
}
export interface RecentProject { id: string; name: string; family: string; updated: string; path: string; status: string }
export interface Progress { task_id: string; phase: string; current: number; total: number | null; message: string }
export interface FdxError { code: string; message: string; details?: unknown; recovery?: string[] }
export type ResultView = 'findings' | 'changes' | 'history';
export interface ResultQuery { view: ResultView; offset: number; limit: number; query?: string; kind?: string; sort?: 'row' | 'kind'; descending?: boolean }
export interface ResultPage { items: (Finding | CellChange | HistoryEntry)[]; total: number }
export interface Operations {
  create: { payload: { name: string; family: string }; result: Session };
  metadata: { payload: { name: string; family: string }; result: Session };
  demo: { payload: Record<string, never>; result: Session };
  recent: { payload: Record<string, never>; result: RecentProject[] };
  inspect: { payload: { path: string; header_row: number }; result: WorkbookInfo };
  import: { payload: { path: string; dictionary_path?: string; sheet?: string; header_row: number }; result: Session };
  dictionary: { payload: { path?: string; text?: string }; result: Session };
  configure: { payload: { config: ProjectConfig }; result: Session };
  analyze: { payload: Record<string, never>; result: Session };
  results: { payload: ResultQuery; result: ResultPage };
  decide: { payload: { decisions: Decisions }; result: Session };
  undo: { payload: Record<string, never>; result: Session };
  redo: { payload: Record<string, never>; result: Session };
  profile_load: { payload: { path: string }; result: Session };
  profile_save: { payload: { path: string }; result: { path: string } };
  project_open: { payload: { path: string }; result: Session };
  project_save: { payload: { path: string; ui_state?: unknown }; result: { path: string } };
  export: { payload: { path: string; allow_issues: boolean; overwrite: boolean }; result: { path: string } };
  open_path: { payload: { path: string; folder?: boolean }; result: null };
}
export type Op = keyof Operations;
export type Request<K extends Op = Op> = { op: K; task_id: string; session_id?: string; revision?: number; payload: Operations[K]['payload'] };
export const sessionOps = new Set<Op>(['create', 'metadata', 'demo', 'import', 'dictionary', 'configure', 'analyze', 'decide', 'undo', 'redo', 'profile_load', 'project_open']);
export const readOps = new Set<Op>(['recent', 'inspect', 'results', 'open_path']);
export const cell = (value = '', kind = 'text'): CellValue => ({ kind, value });
export const emptyCell = (): CellValue => cell('', 'empty');
export const newField = (): FieldSpec => ({ code: '', label: '', dtype: 'text', role: 'standard', max_length: null, date_format: 'DD/MM/YYYY', required: false, pattern: '', allowed_values: [], minimum: null, maximum: null });
export const defaultRule = (): FieldRule => ({ normalize_spaces: true, convert: true, decimal_separator: ',', thousands_separator: ' ', currency_tokens: ['FCFA', 'XAF'], date_order: 'dmy', phone_region: 'CM', phone_policy: 'report', phone_separator: '/', length_policy: 'report', invalid_policy: 'report', replacements: {} });
export const defaultConfig = (): ProjectConfig => ({ version: 2, source_path: '', original_path: '', dictionary_path: '', sheet: '', header_row: 1, fields: [], mappings: [], rules: {}, chronology: [], segmentation: { enabled: false, source_column: 1, name_column: null, bp_column: null, phone_column: null, location_column: null, overwrite: false }, harmonized: true, report_collision: 'suffix', reports: ['control', 'original', 'verify'] });
export const filename = (path: string) => path.split(/[\\/]/).pop() || path;
export function analysisConfig(config: ProjectConfig): string {
  const { harmonized: _harmonized, reports: _reports, report_collision: _collision, ...analysis } = config;
  return JSON.stringify(analysis);
}
export function columnName(position: number): string {
  let label = '';
  for (let n = position; n > 0; n = Math.floor((n - 1) / 26)) label = String.fromCharCode(65 + (n - 1) % 26) + label;
  return label;
}
export function normalizeError(error: unknown): FdxError {
  if (typeof error === 'string') { try { return normalizeError(JSON.parse(error)); } catch { return { code: 'FDX_ERROR', message: 'L’opération n’a pas abouti.', details: error }; } }
  if (error && typeof error === 'object' && 'code' in error && 'message' in error) return error as FdxError;
  return { code: 'UI_ERROR', message: 'L’opération n’a pas abouti. Les données sources restent inchangées.', details: error instanceof Error ? error.message : error };
}
export function workbookReadError(error: unknown): FdxError {
  const original = normalizeError(error);
  if (original.code.toLowerCase() === 'cancelled') return original;
  return {
    code: 'WORKBOOK_READ', message: original.message,
    details: { code: original.code, ...(original.details !== undefined ? { detail: original.details } : {}) },
    recovery: [
      'Ouvrez le classeur dans Microsoft Excel ou LibreOffice Calc pour vérifier son contenu.',
      'Si nécessaire, réparez le fichier ou retirez sa protection. Vérifiez aussi vos droits d’accès et fermez le fichier s’il est verrouillé.',
      'Avant toute conversion, vérifiez qu’aucune donnée, macro ou objet nécessaire n’est perdu et conservez l’original.',
      'Enregistrez une nouvelle copie au format .xlsx, fermez-la dans le tableur, puis rechargez cette copie dans FDX Atelier.',
    ],
  };
}
export function configIssues(config: ProjectConfig): string[] {
  const errors: string[] = [];
  const codes = config.fields.map(f => f.code.trim());
  if (!codes.length || codes.some(c => !c) || new Set(codes).size !== codes.length) errors.push('Le dictionnaire doit contenir des codes non vides et uniques.');
  const active = config.mappings.filter(m => m.field_code);
  if (!active.length) errors.push('Associez au moins une colonne.');
  if (active.some(m => !m.confirmed || !codes.includes(m.field_code!))) errors.push('Confirmez chaque association active.');
  if (new Set(active.map(m => m.field_code)).size !== active.length) errors.push('Un champ ne peut être associé qu’à une seule colonne.');
  if (new Set(config.mappings.map(m => m.column)).size !== config.mappings.length) errors.push('Une position Excel est répétée.');
  for (const f of config.fields) {
    if (f.required && !active.some(m => m.field_code === f.code)) errors.push(`Champ obligatoire sans colonne : ${f.code}.`);
    for (const bound of [f.minimum, f.maximum]) if (bound !== null && !/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(bound)) errors.push(`Borne décimale invalide : ${f.code} (point décimal).`);
  }
  if (!config.source_path || !config.sheet) errors.push('Importez un classeur et confirmez sa feuille.');
  if (config.segmentation.enabled) {
    const s = config.segmentation;
    const targets = [s.name_column, s.bp_column, s.phone_column, s.location_column].filter(v => v !== null);
    if (!targets.length || new Set(targets).size !== targets.length) errors.push('Choisissez des destinations distinctes pour la segmentation.');
  }
  for (const r of config.chronology) if (r.enabled && (!active.some(m => m.field_code === r.before) || !active.some(m => m.field_code === r.after) || r.before === r.after)) errors.push('Une relation chronologique exige deux champs associés distincts.');
  return [...new Set(errors)];
}
