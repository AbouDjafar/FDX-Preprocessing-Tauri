import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { Check, X } from 'lucide-react';
import { analysisConfig, configIssues, normalizeError, readOps, sessionOps, type FdxError, type Op, type Operations, type Progress, type ProjectConfig, type RecentProject, type Session } from './model';
import { createAdapter, type Adapter } from './services/bridge';
import { Button, Modal } from './components/ui';

type Run = <K extends Op>(op: K, payload: Operations[K]['payload'], quiet?: boolean) => Promise<Operations[K]['result']>;
type Task = { id: string; op: Op; progress: Progress | null; cancelling: boolean };
interface Workspace {
  mode: Adapter['mode']; session: Session | null; draft: ProjectConfig | null; edited: boolean; unsaved: boolean; decisionsReady: boolean;
  edit: (update: (config: ProjectConfig) => ProjectConfig) => void; task: Task | null; busy: boolean;
  error: FdxError | null; reportError: (error: unknown) => void; clearError: () => void;
  run: Run; act: (fn: () => Promise<unknown>) => void; cancel: () => Promise<void>;
  saveDraft: () => Promise<void>; analyze: () => Promise<void>; recent: RecentProject[]; refreshRecent: () => Promise<void>;
  confirm: (title: string, message: string, accept?: string) => Promise<boolean>;
  toast: string; notify: (text: string) => void;
}
const Context = createContext<Workspace | null>(null);
export const useWorkspace = () => { const value = useContext(Context); if (!value) throw new Error('Workspace absent'); return value; };
export function WorkspaceProvider({ children, adapter: injected }: { children: ReactNode; adapter?: Adapter }) {
  const [adapter] = useState(() => injected || createAdapter());
  const [session, setSession] = useState<Session | null>(null);
  const sessionRef = useRef(session);
  const [draft, setDraft] = useState<ProjectConfig | null>(null);
  const draftRef = useRef(draft);
  const [task, setTask] = useState<Task | null>(null);
  const taskRef = useRef<Task | null>(null);
  const locked = useRef(false);
  const [error, setError] = useState<FdxError | null>(null);
  const [toast, notify] = useState('');
  const [recent, setRecent] = useState<RecentProject[]>([]);
  const [unsaved, setUnsaved] = useState(false);
  const [analyzedConfig, setAnalyzedConfig] = useState<string | null>(null);
  const [question, setQuestion] = useState<{ title: string; message: string; accept: string; resolve: (yes: boolean) => void } | null>(null);
  const questionRef = useRef(question); questionRef.current = question;
  const reportError = useCallback((value: unknown) => setError(normalizeError(value)), []);
  const act = useCallback((fn: () => Promise<unknown>) => { void fn().catch(reportError); }, [reportError]);
  const run: Run = useCallback(async <K extends Op,>(op: K, payload: Operations[K]['payload'], quiet = false): Promise<Operations[K]['result']> => {
    const mutating = !readOps.has(op);
    if (mutating && locked.current) throw { code: 'BUSY', message: 'Attendez la fin de l’opération en cours.' };
    if (mutating) locked.current = true;
    const base = sessionRef.current;
    const id = crypto.randomUUID();
    if (!quiet) { taskRef.current = { id, op, progress: null, cancelling: false }; setTask(taskRef.current); setError(null); }
    try {
      const response = await adapter.request({ op, payload, task_id: id, ...(base ? { session_id: base.id, revision: base.revision } : {}) }, progress => {
        if (progress.task_id === id && taskRef.current?.id === id) { taskRef.current = { ...taskRef.current, progress }; setTask(taskRef.current); }
      });
      if (sessionOps.has(op)) {
        const next = response as Session;
        if (!next || typeof next.id !== 'string' || typeof next.revision !== 'number' || !next.config) throw { code: 'INVALID_RESPONSE', message: 'Réponse du moteur incompatible.', details: { op } };
        if (sessionRef.current?.id !== base?.id || sessionRef.current?.revision !== base?.revision) throw { code: 'STALE_RESPONSE', message: 'Résultat périmé ignoré. Le projet a changé.' };
        if (base?.id === next.id && next.revision < base.revision) throw { code: 'STALE_RESPONSE', message: 'Révision périmée ignorée.' };
        sessionRef.current = next; setSession(next); draftRef.current = structuredClone(next.config); setDraft(draftRef.current); setUnsaved(op !== 'project_open');
        if (op === 'analyze') setAnalyzedConfig(analysisConfig(next.config));
        else if (['import', 'demo', 'create', 'project_open'].includes(op)) setAnalyzedConfig(null);
      } else if (op === 'project_save' || op === 'export') {
        if (sessionRef.current && sessionRef.current.id === base?.id) {
          const path = (response as { path: string }).path;
          const next = { ...sessionRef.current, ...(op === 'export' ? { export_path: path } : { project_path: path }) };
          sessionRef.current = next; setSession(next);
          setUnsaved(op === 'export');
        }
      }
      return response;
    } catch (value) { if (!quiet) reportError(value); throw value; }
    finally { if (mutating) locked.current = false; if (taskRef.current?.id === id) { taskRef.current = null; setTask(null); } }
  }, [adapter, reportError]);
  const refreshRecent = useCallback(async () => { setRecent(await run('recent', {}, true)); }, [run]);
  useEffect(() => { act(refreshRecent); }, [act, refreshRecent]);
  useEffect(() => { if (!toast) return; const timer = setTimeout(() => notify(''), 6000); return () => clearTimeout(timer); }, [toast]);
  useEffect(() => () => { questionRef.current?.resolve(false); }, []);
  const edit = useCallback((update: (config: ProjectConfig) => ProjectConfig) => {
    if (!draftRef.current || locked.current) return;
    draftRef.current = update(draftRef.current); setDraft(draftRef.current);
  }, []);
  const saveDraft = useCallback(async () => {
    if (!draftRef.current || !sessionRef.current) return;
    if (JSON.stringify(draftRef.current) !== JSON.stringify(sessionRef.current.config)) await run('configure', { config: draftRef.current });
  }, [run]);
  const analyze = useCallback(async () => {
    if (!draftRef.current) return;
    const issues = configIssues(draftRef.current);
    if (issues.length) throw { code: 'CONFIG_INVALID', message: 'La préparation doit être complétée avant analyse.', details: issues };
    await saveDraft(); await run('analyze', {}); notify('Vérification terminée. Les résultats sont à jour.');
  }, [run, saveDraft]);
  const cancel = useCallback(async () => {
    const active = taskRef.current;
    if (!active || active.cancelling) return;
    taskRef.current = { ...active, cancelling: true }; setTask(taskRef.current);
    try { await adapter.cancel(active.id); }
    catch (value) { if (taskRef.current?.id === active.id) { taskRef.current = { ...taskRef.current, cancelling: false }; setTask(taskRef.current); } throw value; }
  }, [adapter]);
  const confirm = useCallback((title: string, message: string, accept = 'Confirmer') => new Promise<boolean>(resolve => {
    questionRef.current?.resolve(false); setQuestion({ title, message, accept, resolve });
  }), []);
  const answer = (yes: boolean) => { question?.resolve(yes); setQuestion(null); };
  const edited = !!draft && !!session && JSON.stringify(draft) !== JSON.stringify(session.config);
  const decisionsReady = !!session?.summary && !!draft && analysisConfig(draft) === analyzedConfig;
  useEffect(() => {
    const guard = (e: BeforeUnloadEvent) => { if (edited || unsaved || task) { e.preventDefault(); e.returnValue = ''; } };
    window.addEventListener('beforeunload', guard); return () => window.removeEventListener('beforeunload', guard);
  }, [edited, unsaved, task]);
  return <Context.Provider value={{ mode: adapter.mode, session, draft, edited, unsaved, decisionsReady, edit, task, busy: !!task, error, reportError, clearError: () => setError(null), run, act, cancel, saveDraft, analyze, recent, refreshRecent, confirm, toast, notify }}>
    {children}
    {question && <Modal title={question.title} onClose={() => answer(false)}><p className="dialog-message">{question.message}</p><footer className="modal-footer"><Button icon={X} onClick={() => answer(false)}>Annuler</Button><Button icon={Check} primary onClick={() => answer(true)}>{question.accept}</Button></footer></Modal>}
  </Context.Provider>;
}
