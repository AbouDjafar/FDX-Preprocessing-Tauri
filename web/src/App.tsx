import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { ArrowLeft, ArrowRight, ClipboardCheck, FileCheck2, FolderOpen, Home as HomeIcon, ListChecks, Plus, Redo2, Save, ShieldCheck, SlidersHorizontal, Undo2, Upload, X } from 'lucide-react';
import { version } from '../package.json';
import { filename } from './model';
import { selectFile, subscribeClose } from './services/bridge';
import { useWorkspace } from './workspace';
import { Button, Empty, Field, IconButton, Modal } from './components/ui';
import { Home } from './components/Home';
import { Feedback } from './components/Feedback';
import { ImportStage } from './stages/ImportStage';
import { MappingStage } from './stages/MappingStage';
import { RulesStage } from './stages/RulesStage';
import { ReviewStage } from './stages/ReviewStage';
import { ExportStage } from './stages/ExportStage';

const steps = [
  { slug: 'import', label: 'Importer', title: 'Importer les sources', icon: Upload },
  { slug: 'association', label: 'Associer', title: 'Associer les colonnes', icon: SlidersHorizontal },
  { slug: 'traitements', label: 'Règles', title: 'Confirmer les règles', icon: ListChecks },
  { slug: 'anomalies', label: 'Anomalies', title: 'Examiner les résultats', icon: ClipboardCheck },
  { slug: 'export', label: 'Exporter', title: 'Préparer le reporting', icon: FileCheck2 },
] as const;
type Stage = typeof steps[number]['slug'];
type ProjectDetails = { name: string; family: string };
type LeaveChoice = 'save' | 'discard' | 'cancel';
const subscribeHash = (notify: () => void) => { window.addEventListener('hashchange', notify); return () => window.removeEventListener('hashchange', notify); };
const readHash = () => window.location.hash.slice(1) || '/';
const go = (path: string) => { window.location.hash = path; };

export default function App() {
  const w = useWorkspace();
  const route = useSyncExternalStore(subscribeHash, readHash, () => '/');
  const match = /^\/projet\/([^/]+)\/([^/]+)$/.exec(route);
  const stage = match?.[2] as Stage | undefined;
  const routeSession = match?.[1];
  const validSession = !!w.session && routeSession === w.session.id;
  const currentStep = steps.find(s => s.slug === stage);
  const [advanced, setAdvanced] = useState(false);
  const [name, setName] = useState(''); const [family, setFamily] = useState('');
  const [detailsQuestion, setDetailsQuestion] = useState<{ resolve: (details: ProjectDetails | null) => void } | null>(null);
  const [leaveQuestion, setLeaveQuestion] = useState<{ resolve: (choice: LeaveChoice) => void } | null>(null);
  const switching = useRef(false);
  const main = useRef<HTMLElement>(null);
  const navigate = (next: Stage, id = w.session?.id) => { if (id && !w.busy) go(`/projet/${id}/${next}`); };
  // Async actions navigate using the returned id; the render's busy flag can be stale.
  const navigateResult = (next: Stage, id: string) => go(`/projet/${id}/${next}`);
  const requestDetails = () => new Promise<ProjectDetails | null>(resolve => {
    setName(w.session?.name === 'Projet sans titre' ? filename(w.session.config.original_path || w.session.config.source_path).replace(/\.[^.]+$/, '') || 'Projet FDX' : w.session?.name || 'Projet FDX');
    setFamily(w.session?.family === 'À préciser' ? '' : w.session?.family || '');
    setDetailsQuestion({ resolve });
  });
  const answerDetails = (details: ProjectDetails | null) => { detailsQuestion?.resolve(details); setDetailsQuestion(null); };
  const answerLeave = (choice: LeaveChoice) => { leaveQuestion?.resolve(choice); setLeaveQuestion(null); };
  const saveProject = async (): Promise<boolean> => {
    if (!w.session) return false;
    if (!w.session.workbook) throw { code: 'PROJECT_NO_SOURCE', message: 'Confirmez d’abord le classeur et sa feuille avec « Importer et associer », puis enregistrez le projet.' };
    const details = await requestDetails(); if (!details) return false;
    const suggested = `${details.name.replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_')}_copie.fdxproj`;
    const path = await selectFile(w.mode, 'project', true, suggested); if (!path) return false;
    if (path === w.session.project_path && !await w.confirm('Remplacer le projet enregistré ?', 'Cette destination correspond au projet ouvert. Confirmez explicitement son remplacement ou choisissez une nouvelle copie.', 'Remplacer ce projet')) return false;
    await w.saveDraft();
    await w.run('metadata', details);
    await w.run('project_save', { path, ui_state: { stage: stage || 'import', mode: advanced ? 'advanced' : 'guided' } }); await w.refreshRecent();
    w.notify(w.mode === 'demo' ? 'Projet DEMO enregistré en mémoire. Aucun fichier créé.' : 'Projet enregistré.');
    return true;
  };
  const maySwitch = async () => {
    if (!(w.unsaved || w.edited)) return true;
    const choice = await new Promise<LeaveChoice>(resolve => setLeaveQuestion({ resolve }));
    return choice === 'discard' || (choice === 'save' && await saveProject());
  };
  const switchProject = async (action: () => Promise<void>) => {
    if (switching.current || w.busy) return;
    switching.current = true;
    try { if (await maySwitch()) await action(); } finally { switching.current = false; }
  };
  const demo = () => w.act(() => switchProject(async () => { const s = await w.run('demo', {}); navigateResult('import', s.id); }));
  const openProject = (knownPath?: string) => w.act(async () => {
    const path = knownPath || await selectFile(w.mode, 'project'); if (!path) return;
    await switchProject(async () => {
      const s = await w.run('project_open', { path });
      const ui = s.ui_state && typeof s.ui_state === 'object' ? s.ui_state as Record<string, unknown> : {};
      setAdvanced(ui.mode === 'advanced'); navigateResult(steps.some(v => v.slug === ui.stage) ? ui.stage as Stage : 'association', s.id);
    });
  });
  const create = () => w.act(() => switchProject(async () => {
    const s = await w.run('create', { name: 'Projet sans titre', family: 'À préciser' });
    setAdvanced(false); navigateResult('import', s.id);
  }));
  const closeHandler = useRef(async () => true);
  closeHandler.current = async () => {
    if (w.busy || switching.current || document.querySelector('dialog[open]')) { w.notify('Terminez ou fermez la fenêtre en cours avant de quitter.'); return false; }
    return maySwitch();
  };
  useEffect(() => {
    if (w.mode !== 'desktop') return;
    let disposed = false; let unlisten: (() => void) | undefined;
    void subscribeClose(() => closeHandler.current(), w.reportError).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(w.reportError);
    return () => { disposed = true; unlisten?.(); };
  }, [w.mode, w.reportError]);
  const shortcut = useRef((event: KeyboardEvent) => {});
  shortcut.current = event => {
    if (!(event.ctrlKey || event.metaKey) || event.altKey || w.busy || document.querySelector('dialog[open]')) return;
    const target = event.target as HTMLElement;
    if (target.closest('input,textarea,select,[contenteditable="true"]')) return;
    if (event.key.toLowerCase() === 's' && w.session) { event.preventDefault(); w.act(saveProject); }
    if (event.key.toLowerCase() === 'z' && w.session && !w.edited) {
      const redo = event.shiftKey;
      if (redo ? w.session.can_redo : w.session.can_undo) { event.preventDefault(); w.act(() => w.run(redo ? 'redo' : 'undo', {})); }
    }
  };
  useEffect(() => { const handler = (event: KeyboardEvent) => shortcut.current(event); window.addEventListener('keydown', handler); return () => window.removeEventListener('keydown', handler); }, []);
  useEffect(() => {
    window.scrollTo({ top: 0, left: 0, behavior: 'instant' });
    main.current?.scrollTo({ top: 0, left: 0, behavior: 'instant' });
    main.current?.focus({ preventScroll: true });
    document.title = `${currentStep?.title || 'Espace de travail'} · FDX Atelier`;
  }, [route, currentStep?.title]);
  return <div className="app-shell">
    <a href="#main-content" className="skip-link" onClick={e => { e.preventDefault(); main.current?.focus(); }}>Aller au contenu</a>
    <aside className="sidebar">
      <button className="brand" disabled={w.busy} onClick={() => go('/')} aria-label="FDX Atelier, accueil"><span className="brand-mark">F</span><span><span className="brand-name">FDX Atelier</span><span className="brand-sub">FIABILITÉ DES DONNÉES</span></span></button>
      <nav aria-label="Navigation principale"><div className="nav-label">ESPACE DE TRAVAIL</div><button className={`nav-link ${route === '/' ? 'active' : ''}`} aria-current={route === '/' ? 'page' : undefined} disabled={w.busy} onClick={() => go('/')}><HomeIcon size={16} />Vue d’ensemble</button>
        <div className="nav-label">PROJET EN COURS</div>{w.session && <div className="project-mini"><span>PROJET ACTIF</span><strong>{w.session.name}</strong><span>{filename(w.session.config.source_path) || 'Sans source'}</span></div>}
        {steps.map(s => <button key={s.slug} className={`nav-link ${validSession && stage === s.slug ? 'active' : ''}`} aria-current={validSession && stage === s.slug ? 'step' : undefined} data-testid={`nav-${s.slug}`} disabled={w.busy || !w.session || (s.slug !== 'import' && !w.session.workbook)} onClick={() => navigate(s.slug)}><s.icon size={16} />{s.label}</button>)}
      </nav>
      <div className="sidebar-bottom"><ShieldCheck size={24} aria-hidden="true" /><div>Source préservée<span>{w.mode === 'demo' ? 'DEMO · session en mémoire' : 'Traitement local sur cet appareil'}</span></div></div>
    </aside>
    <div className="workspace">
      <header className="topbar"><div className="crumb"><b>{validSession ? w.session?.name : 'Espace de travail'}</b>{validSession && <span>{currentStep?.label}</span>}</div><div className="actions"><span className="app-version" data-testid="app-version">v{version}</span><span className={w.mode === 'demo' ? 'demo-chip' : 'local-chip'}>{w.mode === 'demo' ? 'DEMO · DONNÉES FICTIVES' : 'BUREAU · LOCAL'}</span></div></header>
      <main id="main-content" className="main" ref={main} tabIndex={-1}>
        {route === '/' ? <Home create={create} demo={demo} open={openProject} resume={() => navigate(w.session?.workbook ? 'association' : 'import')} /> : !validSession ? <Empty title="Aucun projet ouvert pour cette adresse"><p>Les sessions ne sont pas restaurées automatiquement après rechargement. Ouvrez votre projet enregistré depuis l’accueil.</p><Button icon={ArrowLeft} onClick={() => go('/')}>Retour à l’accueil</Button></Empty> : !currentStep ? <Empty title="Étape introuvable"><Button onClick={() => navigate('import')}>Revenir au projet</Button></Empty> : <>
          <div className="project-header" data-testid="project-header">
          <div className="title-row"><div><div className="eyebrow">{w.session!.family} · SOURCE INTACTE</div><h1 className="page-title">{currentStep.title}</h1><p className="lead mono">{filename(w.session!.config.original_path || w.session!.config.source_path) || 'Nouveau projet'}</p></div><div className="actions"><IconButton icon={Plus} label="Nouveau projet" disabled={w.busy} onClick={create} /><IconButton icon={Undo2} label="Annuler les décisions (Ctrl+Z)" disabled={w.busy || w.edited || !w.session!.can_undo} onClick={() => w.act(() => w.run('undo', {}))} /><IconButton icon={Redo2} label="Rétablir les décisions (Ctrl+Maj+Z)" disabled={w.busy || w.edited || !w.session!.can_redo} onClick={() => w.act(() => w.run('redo', {}))} /><Button icon={Save} disabled={w.busy} onClick={() => w.act(saveProject)} data-testid="save-project">Enregistrer une copie</Button>{w.session!.project_path && w.mode === 'desktop' && <IconButton icon={FolderOpen} label="Ouvrir le dossier du projet enregistré" disabled={w.busy} onClick={() => w.act(() => w.run('open_path', { path: w.session!.project_path!, folder: true }))} />}</div></div>
          <div className="project-tools"><div className="segmented" role="group" aria-label="Mode de configuration"><button aria-pressed={!advanced} className={!advanced ? 'active' : ''} onClick={() => setAdvanced(false)}>Guidé</button><button aria-pressed={advanced} className={advanced ? 'active' : ''} onClick={() => setAdvanced(true)}>Avancé</button></div><span className="draft-status">{w.edited ? 'Paramètres modifiés' : w.unsaved ? 'Projet non enregistré' : 'Projet enregistré'}</span></div>
          <nav className="stepper" aria-label="Étapes du projet">{steps.map((s, i) => <button key={s.slug} data-testid={`stage-${s.slug}`} className={`step ${stage === s.slug ? 'current' : ''}`} aria-current={stage === s.slug ? 'step' : undefined} disabled={w.busy || (s.slug !== 'import' && !w.session!.workbook)} onClick={() => navigate(s.slug)}><span className="step-num">{String(i + 1).padStart(2, '0')}</span><span>{s.label}</span></button>)}</nav>
          </div>
          <nav className="stage-nav" aria-label="Passer d’une étape à une autre">
            <Button icon={ArrowLeft} disabled={w.busy || stage === steps[0].slug} onClick={() => navigate(steps[steps.findIndex(s => s.slug === stage) - 1].slug)}>Étape précédente</Button>
            <span className="mono">Étape {steps.findIndex(s => s.slug === stage) + 1} sur {steps.length}</span>
            <Button icon={ArrowRight} disabled={w.busy || stage === steps.at(-1)!.slug || !w.session!.workbook} onClick={() => navigate(steps[steps.findIndex(s => s.slug === stage) + 1].slug)}>Étape suivante</Button>
          </nav>
          <fieldset className="stage-content" disabled={w.busy} aria-busy={w.busy} key={w.session!.id}>
            {stage === 'import' && <ImportStage next={() => navigateResult('association', w.session!.id)} />}
            {stage === 'association' && <MappingStage advanced={advanced} next={() => navigateResult('traitements', w.session!.id)} />}
            {stage === 'traitements' && <RulesStage advanced={advanced} next={() => navigateResult('anomalies', w.session!.id)} />}
            {stage === 'anomalies' && <ReviewStage next={() => navigate('export')} />}
            {stage === 'export' && <ExportStage back={() => navigate('anomalies')} />}
          </fieldset>
        </>}
      </main>
    </div>
    <Feedback />
    {leaveQuestion && <Modal title="Enregistrer avant de continuer ?" onClose={() => answerLeave('cancel')}>
      <p>Le projet « {w.session?.name} » contient du travail non enregistré.</p>
      <footer className="modal-footer"><Button icon={X} onClick={() => answerLeave('cancel')}>Annuler</Button><Button onClick={() => answerLeave('discard')}>Continuer sans enregistrer</Button><Button primary icon={Save} onClick={() => answerLeave('save')}>Enregistrer et continuer</Button></footer>
    </Modal>}
    {detailsQuestion && <Modal title="Enregistrer le projet" onClose={() => answerDetails(null)}><form onSubmit={e => { e.preventDefault(); if (name.trim() && family.trim()) answerDetails({ name: name.trim(), family: family.trim() }); }}>
      <Field label="Nom du projet"><input autoFocus required maxLength={200} value={name} onChange={e => setName(e.target.value)} data-testid="project-name" /></Field>
      <Field label="Famille de données"><input list="project-families" required maxLength={100} value={family} onChange={e => setFamily(e.target.value)} /></Field>
      <datalist id="project-families"><option>Marchés signés</option><option>Passation</option><option>Exécution</option></datalist>
      <footer className="modal-footer"><Button onClick={() => answerDetails(null)}>Annuler</Button><Button primary icon={Save} type="submit" disabled={!name.trim() || !family.trim() || w.busy}>Choisir l’emplacement</Button></footer>
    </form></Modal>}
  </div>;
}
