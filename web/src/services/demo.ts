import { analysisConfig, cell, configIssues, defaultConfig, emptyCell, newField, readOps, type CellChange, type Decisions, type FieldSpec, type Finding, type HistoryEntry, type Op, type Operations, type Progress, type ProjectConfig, type RecentProject, type Request, type Session, type WorkbookInfo } from '../model';
import type { Adapter } from './bridge';

export const DEMO_SOURCE = '/DEMO/donnees-synthetiques/marches-fictifs.xlsx';
export const DEMO_DICTIONARY = '/DEMO/donnees-synthetiques/dictionnaire-fictif.md';
const copy = <T,>(value: T): T => structuredClone(value);
const fail = (code: string, message: string): never => { throw { code, message }; };
const fields = (): FieldSpec[] => [
  { ...newField(), code: 'REFERENCE', label: 'Référence', role: 'identifier', required: true, max_length: 40 },
  { ...newField(), code: 'MONTANT', label: 'Montant TTC', dtype: 'decimal', minimum: '0' },
  { ...newField(), code: 'SIGNATURE', label: 'Date de signature', dtype: 'date' },
  { ...newField(), code: 'TELEPHONE', label: 'Téléphone', role: 'phone' },
  { ...newField(), code: 'TITULAIRE', label: 'Titulaire', role: 'company', required: true, max_length: 100 },
  { ...newField(), code: 'FIN', label: 'Date de fin', dtype: 'date' },
  { ...newField(), code: 'PROCEDURE', label: 'Procédure', allowed_values: ['AO', 'Gré à gré'] },
  { ...newField(), code: 'TOTAL', label: 'Total calculé', dtype: 'decimal' },
];
const headers = ['Référence', 'Montant TTC', 'Signature', 'Contact', 'Titulaire', 'Fin', 'Procédure', 'Total', 'Entreprise', 'BP', 'Contact', 'Localisation'];
export const demoWorkbook = (): WorkbookInfo => ({ path: DEMO_SOURCE, suggested_sheet: 'Marchés fictifs', sheets: [
  { name: 'Marchés fictifs', rows: 1201, columns: 12, suggested_header: 1, headers: headers.map(v => cell(v)), preview: [
    ['DEMO/0001', '8.450.000', '03/04/2025', '699 / +237 677000000 / +237 699000000', 'Entreprise Exemple A', '01/04/2025', 'AO ouvert', '=B2', '', '', '', ''].map(v => cell(v)),
    ['DEMO/0002', '12345678901234567890.123456789', '2025-06-15', '+237 677000000', 'Entreprise Exemple B', '2025-07-01', 'AO', '=B3', '', '', '', ''].map((v, i) => cell(v, i === 1 ? 'decimal' : i === 7 ? 'formula' : 'text')),
  ] },
  { name: 'Annexe fictive', rows: 2, columns: 2, suggested_header: 1, headers: [cell('Code'), cell('Note')], preview: [[cell('DEMO'), cell('Annexe conservée dans le scénario')]] },
] });
function fixtureConfig(): ProjectConfig {
  const config = defaultConfig();
  return { ...config, source_path: DEMO_SOURCE, original_path: DEMO_SOURCE, dictionary_path: DEMO_DICTIONARY, sheet: 'Marchés fictifs', fields: fields(), mappings: headers.map((_, i) => ({ column: i + 1, field_code: fields()[i]?.code ?? null, score: i < 8 ? 100 : 0, confirmed: false })) };
}
interface State { session: Session; decisions: Decisions; undo: Decisions[]; redo: Decisions[]; history: HistoryEntry[]; findings: Finding[]; changes: CellChange[] }
function state(name: string, family: string): State {
  return { session: { id: crypto.randomUUID(), name, family, revision: 0, config: defaultConfig(), workbook: null, notes: [], summary: null, dirty: true, pending_decisions: 0, can_undo: false, can_redo: false, export_path: null, project_path: null }, decisions: {}, undo: [], redo: [], history: [], findings: [], changes: [] };
}
function fixtureFinding(index: number, config: ProjectConfig): Finding | null {
  const row = index + config.header_row + 1;
  const scenarios = [
    { column: 2, kind: 'number', message: 'Séparateurs numériques à confirmer', original: cell('8.450.000'), proposed: cell('8450000', 'decimal'), choices: [cell('8450000', 'decimal'), cell('8450', 'decimal')] },
    { column: 3, kind: 'date', message: 'Date ambiguë : jour et mois', original: cell('03/04/2025'), proposed: cell('2025-04-03', 'date'), choices: [cell('2025-04-03', 'date'), cell('2025-03-04', 'date')] },
    { column: 4, kind: 'phone_multiple', message: 'Plusieurs numéros dans une cellule', original: cell('699 / +237 677000000 / +237 699000000'), proposed: null, choices: [cell('699'), cell('+237 677000000'), cell('+237 699000000')] },
    { column: 5, kind: 'required', message: 'Titulaire obligatoire absent', original: emptyCell(), proposed: null, choices: [] },
    { column: 6, kind: 'chronology', message: 'Fin antérieure à la signature', original: cell('2025-04-01', 'date'), proposed: null, choices: [] },
    { column: 7, kind: 'allowed', message: 'Procédure hors liste autorisée', original: cell('AO ouvert'), proposed: cell('AO'), choices: [cell('AO'), cell('Gré à gré')] },
    { column: 8, kind: 'formula', message: 'Formule conservée ; calcul effectué par Excel', original: cell(`=B${row}`, 'formula'), proposed: null, choices: [] },
    { column: 1, kind: 'length', message: 'Identifiant long, zéros initiaux à conserver', original: cell('000012345678901234567890123456789012345678901'), proposed: null, choices: [] },
  ];
  const scenario = scenarios[index % scenarios.length];
  const field_code = config.mappings.find(m => m.column === scenario.column)?.field_code;
  if (!field_code) return null;
  return { ...scenario, key: `${config.sheet}!${row}:${scenario.column}`, row, field_code, current: scenario.original };
}
function analyzeFixture(s: State): void {
  const config = s.session.config;
  const issues = configIssues(config);
  if (issues.length) fail('CONFIG_INVALID', issues.join('\n'));
  s.findings = []; s.changes = [];
  const rows = config.sheet === 'Marchés fictifs' ? 1200 : 1;
  for (let i = 0; i < rows; i++) {
    const finding = fixtureFinding(i, config);
    if (!finding) continue;
    const decision = s.decisions[finding.key];
    if (!decision || decision.action === 'keep' || finding.original.kind === 'formula') { s.findings.push(finding); continue; }
    const after = decision.action === 'clear' ? emptyCell() : decision.value;
    if (JSON.stringify(after) !== JSON.stringify(finding.original)) s.changes.push({ key: finding.key, row: finding.row, column: finding.column, field_code: finding.field_code, before: finding.original, after, reason: decision.reason });
    const spec = config.fields.find(f => f.code === finding.field_code);
    if (after.kind === 'empty' && spec?.required) s.findings.push({ ...finding, current: after, kind: 'required', message: 'Champ obligatoire vide après décision', proposed: null, choices: [] });
    else if (after.value === finding.original.value) s.findings.push({ ...finding, current: after });
  }
  const counts: Record<string, number> = {};
  for (const f of s.findings) counts[f.kind] = (counts[f.kind] || 0) + 1;
  s.session.summary = { row_count: rows, column_count: config.sheet === 'Marchés fictifs' ? 12 : 2, findings: s.findings.length, changes: s.changes.length, counts };
  s.session.pending_decisions = 0;
}

// Synthetic interaction fixture, deliberately not a second spreadsheet engine.
// No file readers, network requests, localStorage or Excel writer exist here.
export class DemoAdapter implements Adapter {
  readonly mode = 'demo' as const;
  private states = new Map<string, State>();
  private projects = new Map<string, State>();
  private profiles = new Map<string, ProjectConfig>();
  private cancelled = new Set<string>();
  private active = new Set<string>();
  constructor(private delay = 80) {}
  async cancel(taskId: string): Promise<void> { if (this.active.has(taskId)) this.cancelled.add(taskId); }
  private assertFixture(path: string): void { if (!path.startsWith('/DEMO/')) fail('DEMO_ONLY', 'Le navigateur accepte uniquement les fixtures DEMO. Aucun fichier privé n’est lu.'); }
  async request<K extends Op>(req: Request<K>, progress: (event: Progress) => void): Promise<Operations[K]['result']> {
    this.active.add(req.task_id);
    try {
      const steps = req.op === 'analyze' ? 6 : 1;
      for (let step = 0; step < steps; step++) {
        progress({ task_id: req.task_id, phase: req.op, current: step, total: steps, message: req.op === 'analyze' ? 'Analyse du scénario synthétique DEMO' : 'Scénario DEMO' });
        await new Promise(resolve => setTimeout(resolve, this.delay));
        if (this.cancelled.has(req.task_id)) fail('CANCELLED', 'Opération annulée. Aucun fichier n’a été écrit.');
      }
      const result = this.perform(req);
      progress({ task_id: req.task_id, phase: req.op, current: steps, total: steps, message: 'Terminé · DEMO' });
      return copy(result) as Operations[K]['result'];
    } finally { this.active.delete(req.task_id); this.cancelled.delete(req.task_id); }
  }
  private perform(req: Request): unknown {
    const previous = req.session_id ? this.states.get(req.session_id) : undefined;
    if (previous && !readOps.has(req.op) && req.revision !== previous.session.revision) fail('REVISION_CONFLICT', 'Le projet a changé. Rechargez le projet avant de continuer.');
    if (req.op === 'recent') return [...this.projects.entries()].map(([path, s]): RecentProject => ({ id: s.session.id, name: s.session.name, family: s.session.family, updated: String(s.history.at(-1)?.at || ''), path, status: 'DEMO en mémoire' }));
    if (req.op === 'inspect') { this.assertFixture((req.payload as Operations['inspect']['payload']).path); return demoWorkbook(); }
    if (req.op === 'create' || req.op === 'demo') {
      const p = req.payload as Operations['create']['payload'];
      const s = state(req.op === 'demo' ? 'Marchés fictifs · Atelier DEMO' : p.name, req.op === 'demo' ? 'Marchés signés' : p.family);
      if (req.op === 'demo') { s.session.config = fixtureConfig(); s.session.workbook = demoWorkbook(); s.session.notes = ['Scénario synthétique de 1 200 lignes. Résultats illustratifs, pas de traitement Excel.']; }
      this.states.set(s.session.id, s); return s.session;
    }
    if (req.op === 'project_open') {
      const { path } = req.payload as Operations['project_open']['payload']; this.assertFixture(path);
      const saved = this.projects.get(path);
      if (!saved) fail('NOT_FOUND', 'Aucun projet DEMO enregistré à cet emplacement en mémoire.');
      const s = copy(saved!); s.session.id = crypto.randomUUID(); s.session.summary = null; s.session.export_path = null; s.session.revision = 0; s.session.dirty = true;
      this.states.set(s.session.id, s); return s.session;
    }
    if (!previous) fail('SESSION_MISSING', 'Ouvrez ou créez un projet.');
    // Work on a copy: failed operations must not partly mutate a session.
    const s = copy(previous!);
    const p = req.payload;
    switch (req.op) {
      case 'metadata': {
        const { name, family } = p as Operations['metadata']['payload'];
        if (!name.trim() || name.trim().length > 200 || !family.trim() || family.trim().length > 100) fail('METADATA_INVALID', 'Renseignez un nom et une famille de données valides.');
        s.session.name = name.trim(); s.session.family = family.trim(); break;
      }
      case 'import': {
        const { path, sheet, header_row, dictionary_path } = p as Operations['import']['payload']; this.assertFixture(path);
        if (dictionary_path) this.assertFixture(dictionary_path);
        if (!demoWorkbook().sheets.some(v => v.name === sheet)) fail('SHEET_MISSING', 'Confirmez une feuille du classeur fictif.');
        s.session.config = { ...fixtureConfig(), sheet: sheet!, header_row };
        s.session.workbook = demoWorkbook(); s.decisions = {}; s.undo = []; s.redo = []; s.findings = []; s.changes = []; s.session.pending_decisions = 0; s.session.summary = null;
        s.session.notes = ['DEMO : seules les données synthétiques intégrées sont utilisées.']; break;
      }
      case 'dictionary': {
        const { path, text } = p as Operations['dictionary']['payload'];
        if (text) fail('DEMO_ONLY', 'Le collage de dictionnaires réels est réservé au bureau. Le dictionnaire fictif reste éditable champ par champ.');
        this.assertFixture(path || ''); s.session.config.fields = fields();
        s.session.config.mappings = fixtureConfig().mappings; s.session.config.dictionary_path = DEMO_DICTIONARY; break;
      }
      case 'configure': {
        const config = copy((p as Operations['configure']['payload']).config);
        const old = s.session.config;
        s.session.config = { ...config, source_path: old.source_path, original_path: old.original_path, dictionary_path: old.dictionary_path }; break;
      }
      case 'analyze': analyzeFixture(s); break;
      case 'results': {
        const { view, offset, limit, query = '', kind, sort = 'row', descending } = p as Operations['results']['payload'];
        let items: (Finding | CellChange | HistoryEntry)[] = view === 'findings' ? s.findings : view === 'changes' ? s.changes : s.history;
        items = items.filter(item => JSON.stringify(item).toLocaleLowerCase('fr').includes(query.toLocaleLowerCase('fr')) && (!kind || ('kind' in item && item.kind === kind)));
        if (view !== 'history') items.sort((a, b) => {
          const primary = sort === 'kind' ? String('kind' in a ? a.kind : '').localeCompare(String('kind' in b ? b.kind : '')) : Number(a.row) - Number(b.row);
          return (primary || Number(a.column) - Number(b.column)) * (descending ? -1 : 1);
        });
        else if (descending) items.reverse();
        return { items: items.slice(offset, offset + limit), total: items.length };
      }
      case 'decide': {
        const decisions = (p as Operations['decide']['payload']).decisions;
        for (const [key, d] of Object.entries(decisions)) {
          if (!s.findings.some(f => f.key === key)) fail('UNKNOWN_CELL', 'Cette cellule n’appartient pas aux résultats du scénario.');
          if (!['keep', 'set', 'clear'].includes(d.action)) fail('INVALID_DECISION', 'Décision inconnue.');
          if (s.findings.some(f => f.key === key && f.original.kind === 'formula') && d.action !== 'keep') fail('FORMULA_PROTECTED', 'Une formule ne peut pas être remplacée.');
        }
        s.undo.push(copy(s.decisions)); s.redo = []; Object.assign(s.decisions, decisions);
        s.session.pending_decisions += Object.keys(decisions).length;
        for (const [key, decision] of Object.entries(decisions)) s.history.push({ at: new Date().toISOString(), op: 'decide', key, decision });
        break;
      }
      case 'undo': case 'redo': {
        const from = req.op === 'undo' ? s.undo : s.redo;
        const to = req.op === 'undo' ? s.redo : s.undo;
        if (!from.length) fail('NO_HISTORY', 'Aucune décision à rétablir ou annuler.');
        to.push(copy(s.decisions)); s.decisions = from.pop()!; s.session.pending_decisions++; break;
      }
      case 'profile_save': {
        const { path } = p as Operations['profile_save']['payload']; this.assertFixture(path);
        this.profiles.set(path, { ...copy(s.session.config), source_path: '', original_path: '', dictionary_path: '' }); return { path };
      }
      case 'profile_load': {
        const { path } = p as Operations['profile_load']['payload']; this.assertFixture(path);
        const config = copy(this.profiles.get(path) || fixtureConfig());
        s.session.config = { ...config, source_path: s.session.config.source_path, original_path: s.session.config.original_path, dictionary_path: s.session.config.dictionary_path, sheet: s.session.config.sheet, header_row: s.session.config.header_row, mappings: config.mappings.map(m => ({ ...m, confirmed: false })) }; break;
      }
      case 'project_save': {
        const { path, ui_state } = p as Operations['project_save']['payload']; this.assertFixture(path);
        s.session.project_path = path; if (ui_state !== undefined) s.session.ui_state = copy(ui_state);
        this.projects.set(path, copy(s)); this.states.set(s.session.id, s); return { path };
      }
      case 'export': return fail('DEMO_ONLY', 'Aucun export Excel réel dans le navigateur. Utilisez l’application de bureau.');
      case 'open_path': return fail('DEMO_ONLY', 'Les chemins DEMO sont synthétiques et ne correspondent à aucun fichier local.');
      default: return fail('UNKNOWN_OPERATION', 'Opération inconnue.');
    }
    const outputOnly = req.op === 'metadata' || (req.op === 'configure' && analysisConfig(s.session.config) === analysisConfig(previous!.session.config));
    if (req.op === 'analyze') s.session.dirty = false;
    else if (!outputOnly) s.session.dirty = true;
    s.session.revision++; if (req.op !== 'metadata') s.session.export_path = null;
    s.session.can_undo = !!s.undo.length; s.session.can_redo = !!s.redo.length;
    if (req.op !== 'decide') s.history.push({ at: new Date().toISOString(), op: req.op, revision: s.session.revision });
    this.states.set(s.session.id, s);
    return s.session;
  }
}
