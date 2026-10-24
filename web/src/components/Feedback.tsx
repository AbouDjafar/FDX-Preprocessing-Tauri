import { AlertTriangle, CheckCircle2, LoaderCircle, X } from 'lucide-react';
import { useWorkspace } from '../workspace';
import { Button, IconButton } from './ui';

const phases: Record<string, string> = { analyze: 'Analyse du classeur', import: 'Import des sources', inspect: 'Lecture de la structure', configure: 'Enregistrement des paramètres', export: 'Export et vérification', project_save: 'Enregistrement du projet', project_open: 'Ouverture du projet', demo: 'Préparation de l’exemple', decide: 'Enregistrement des décisions', undo: 'Annulation du lot', redo: 'Rétablissement du lot' };
export function Feedback() {
  const w = useWorkspace();
  const progress = w.task?.progress;
  const total = progress?.total;
  const ratio = total && total > 0 ? Math.min(100, Math.max(0, progress!.current / total * 100)) : undefined;
  return <>
    {w.error && <div className="global-error" role="alert" data-testid="global-error"><AlertTriangle size={20} aria-hidden="true" /><div className="grow"><strong>{w.error.message}</strong><details><summary>Détails techniques</summary><pre>{w.error.code}{w.error.details !== undefined ? `\n${typeof w.error.details === 'string' ? w.error.details : JSON.stringify(w.error.details, null, 2)}` : ''}</pre></details></div><IconButton icon={X} label="Fermer le message d’erreur" onClick={w.clearError} /></div>}
    {w.task && <section className="task-progress" role="status" aria-live="polite" data-testid="task-progress"><LoaderCircle className="spin" size={22} aria-hidden="true" /><div className="grow"><strong>{phases[w.task.op] || 'Opération en cours'}</strong><p>{w.task.cancelling ? 'Annulation demandée. En attente du moteur…' : progress?.message || 'Préparation…'}</p><progress aria-label="Progression du traitement" max={100} value={ratio} />{ratio !== undefined && <span className="mono">{Math.round(ratio)} %</span>}</div><Button icon={X} disabled={w.task.cancelling} onClick={() => w.act(w.cancel)}>Annuler</Button></section>}
    {w.toast && <div className="toast" role="status"><CheckCircle2 size={18} aria-hidden="true" /><span>{w.toast}</span></div>}
  </>;
}
