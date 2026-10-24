import { ArrowRight, FolderOpen, Plus, RefreshCw, ShieldCheck } from 'lucide-react';
import { filename } from '../model';
import { useWorkspace } from '../workspace';
import { Button, Empty, IconButton, Notice, Section } from './ui';

export function Home({ create, demo, open, resume }: { create: () => void; demo: () => void; open: (path?: string) => void; resume: () => void }) {
  const w = useWorkspace();
  return <>
    <div className="title-row"><div><div className="eyebrow">ESPACE DE TRAVAIL · {w.mode === 'demo' ? 'DEMO' : 'LOCAL'}</div><h1 className="page-title">Bonjour, l’atelier est prêt.</h1><p className="lead">Préparation des classeurs de marchés publics.</p></div><Button primary icon={Plus} disabled={w.busy} onClick={create} data-testid="create-project">Nouveau projet</Button></div>
    <Notice warm={w.mode === 'demo'}>{w.mode === 'demo' ? <><strong>DEMO · données synthétiques.</strong> Aucun fichier privé n’est lu ou transmis. Aucun export Excel réel. Les projets et profils fictifs restent en mémoire pendant cette visite.</> : <><strong>Votre source reste intacte.</strong> Les traitements s’effectuent sur cet appareil. Chaque décision reste traçable.</>}</Notice>
    <div className="home-actions"><Button icon={FolderOpen} disabled={w.busy} onClick={() => open()}>Ouvrir un projet</Button><Button icon={ArrowRight} disabled={w.busy} onClick={demo} data-testid="try-demo">Essayer l’exemple fictif</Button></div>
    {w.session && <section className="active-project"><div><div className="eyebrow">PROJET ACTIF</div><h2>{w.session.name}</h2><p className="mono">{filename(w.session.config.original_path || w.session.config.source_path) || 'Aucune source importée'}</p></div><Button icon={ArrowRight} disabled={w.busy} onClick={resume}>Reprendre le projet</Button></section>}
    <Section title="Projets récents enregistrés" actions={<IconButton icon={RefreshCw} label="Actualiser les projets récents" disabled={w.busy} onClick={() => w.act(w.refreshRecent)} />}>
      {!w.recent.length ? <Empty title="Aucun projet enregistré"><FolderOpen size={30} aria-hidden="true" /></Empty> : <div className="project-list">{w.recent.map(project => <button className="project-row" key={project.path} disabled={w.busy} onClick={() => open(project.path)}><span><strong>{project.name}</strong><span className="project-meta">{filename(project.path)}</span></span><span className="project-family">{project.family}</span><span className="status neutral">{project.status}</span><span className="project-meta">{project.updated ? new Date(project.updated).toLocaleDateString('fr-FR') : ''}</span><ArrowRight size={16} aria-hidden="true" /></button>)}</div>}
    </Section>
    <div className="trust-line"><ShieldCheck size={18} aria-hidden="true" /><span>Source préservée · décisions explicites · résultat vérifié avant export</span></div>
  </>;
}
