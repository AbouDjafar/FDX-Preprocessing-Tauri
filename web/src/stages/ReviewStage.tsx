import { useEffect, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ArrowRight, Check, Eye, Filter, Redo2, RefreshCw, Search, Undo2 } from 'lucide-react';
import { defaultRule, type CellChange, type Decision, type Decisions, type Finding, type HistoryEntry, type ResultPage, type ResultView } from '../model';
import { decisionFor } from '../decisions';
import { useWorkspace } from '../workspace';
import { DecisionDialog } from '../components/DecisionDialog';
import { Button, Empty, IconButton, Notice, Pager, Section } from '../components/ui';

const labels: Record<ResultView, string> = { findings: 'Anomalies', changes: 'Changements', history: 'Historique' };
export function ReviewStage({ next }: { next: () => void }) {
  const w = useWorkspace(); const session = w.session!;
  const [view, setView] = useState<ResultView>('findings');
  const [query, setQuery] = useState(''); const [search, setSearch] = useState('');
  const [kind, setKind] = useState(''); const [sort, setSort] = useState<'row' | 'kind'>('row'); const [descending, setDescending] = useState(false);
  const [offset, setOffset] = useState(0); const [limit, setLimit] = useState(50);
  const [page, setPage] = useState<ResultPage>({ items: [], total: 0 }); const [loading, setLoading] = useState(false); const [failed, setFailed] = useState(false);
  const [retry, setRetry] = useState(0); const [selected, setSelected] = useState<Set<string>>(new Set());
  const [editing, setEditing] = useState<Finding | null>(null); const [queued, setQueued] = useState<Set<string>>(new Set());
  const [lastSummary, setLastSummary] = useState(session.summary);
  const scroll = useRef<HTMLDivElement>(null); const epoch = useRef(0);
  const virtualizer = useVirtualizer({ count: page.items.length, getScrollElement: () => scroll.current, estimateSize: () => view === 'findings' ? 190 : 116, overscan: 5 });
  const current = !!session.summary && !w.edited;
  const canRead = view === 'history' || !!session.summary || session.pending_decisions > 0;
  useEffect(() => { const timer = setTimeout(() => { setSearch(query); setOffset(0); }, 250); return () => clearTimeout(timer); }, [query]);
  useEffect(() => { if (session.summary) { setLastSummary(session.summary); setQueued(new Set()); } }, [session.summary]);
  useEffect(() => {
    const request = ++epoch.current;
    setSelected(new Set()); setFailed(false);
    if (!canRead) { setPage({ items: [], total: 0 }); setLoading(false); return; }
    setLoading(true);
    void w.run('results', { view, offset, limit, query: search, ...(view === 'findings' && kind ? { kind } : {}), sort, descending }, true).then(result => {
      if (epoch.current !== request) return;
      setPage(result); setLoading(false); scroll.current?.scrollTo({ top: 0 });
      if (offset >= result.total && offset > 0) setOffset(Math.max(0, Math.floor(Math.max(0, result.total - 1) / limit) * limit));
    }).catch(error => { if (epoch.current === request) { setLoading(false); setFailed(true); w.reportError(error); } });
    return () => { epoch.current++; };
  }, [w.run, w.reportError, session.id, session.revision, view, offset, limit, search, kind, sort, descending, canRead, retry]);
  const findings = view === 'findings' ? page.items as Finding[] : [];
  const keys = [...new Set(findings.map(f => f.key))];
  const submit = async (decisions: Decisions) => {
    await w.run('decide', { decisions }); setSelected(new Set());
    setQueued(old => new Set([...old, ...Object.keys(decisions)])); w.notify(`${Object.keys(decisions).length} décision(s) ajoutée(s). Vérifiez le lot pour actualiser les résultats.`);
  };
  const batch = (action: 'keep' | 'proposed' | 'clear') => w.act(async () => {
    const picked = findings.filter(f => selected.has(f.key));
    const decisions: Decisions = {};
    for (const f of picked) {
      const d = decisionFor(f, action, session.config.rules[f.field_code] || defaultRule(), 'Décision groupée utilisateur');
      if (decisions[f.key] && JSON.stringify(decisions[f.key]) !== JSON.stringify(d)) throw { code: 'CONFLICTING_CHOICES', message: 'Plusieurs propositions différentes concernent la même cellule. Décidez individuellement pour cette cellule.' };
      decisions[f.key] = d;
    }
    if (!Object.keys(decisions).length) return;
    if (await w.confirm('Confirmer les décisions groupées ?', `${Object.keys(decisions).length} cellule(s) de cette page : ${action === 'keep' ? 'conserver la source' : action === 'clear' ? 'vider les cellules' : 'accepter les propositions'}. Les décisions précédentes de ces cellules seront remplacées.`, 'Ajouter au lot')) await submit(decisions);
  });
  const switchView = (nextView: ResultView) => { setView(nextView); setOffset(0); setPage({ items: [], total: 0 }); setSelected(new Set()); };
  return <>
    <div className="stat-grid">{[['Lignes analysées', lastSummary?.row_count], ['Anomalies', lastSummary?.findings], ['Changements', lastSummary?.changes], ['Décisions en attente', session.pending_decisions]].map(([label, value]) => <div className="stat" key={label}><div className="stat-label">{label}</div><div className="stat-value">{typeof value === 'number' ? value.toLocaleString('fr-FR') : '—'}</div><div className="stat-note">{!current && label !== 'Décisions en attente' ? 'DERNIÈRE ANALYSE' : w.mode === 'demo' ? 'SCÉNARIO DEMO' : 'PROJET ACTIF'}</div></div>)}</div>
    {!current && <Notice warm>{w.edited ? 'Paramètres modifiés. Une nouvelle analyse est requise avant les décisions et l’export.' : session.pending_decisions ? 'Décisions enregistrées, analyse périmée. Vérifiez le lot avant export.' : 'Aucun résultat actuel. Lancez l’analyse après confirmation des associations.'}</Notice>}
    <Section title="4. Examen des résultats" actions={<div className="actions"><IconButton icon={Undo2} label="Annuler le dernier lot de décisions" disabled={!session.can_undo || w.busy || w.edited} onClick={() => w.act(() => w.run('undo', {}))} /><IconButton icon={Redo2} label="Rétablir le dernier lot de décisions" disabled={!session.can_redo || w.busy || w.edited} onClick={() => w.act(() => w.run('redo', {}))} /><Button primary icon={RefreshCw} disabled={w.busy} onClick={() => w.act(w.analyze)}>{session.pending_decisions ? 'Vérifier le lot' : 'Analyser à nouveau'}</Button></div>}>
      <div className="tabbar" role="tablist" aria-label="Vues des résultats">{(Object.keys(labels) as ResultView[]).map((v, i, all) => <button key={v} id={`tab-${v}`} role="tab" aria-controls="result-panel" aria-selected={view === v} tabIndex={view === v ? 0 : -1} className={`tab ${view === v ? 'active' : ''}`} onClick={() => switchView(v)} onKeyDown={e => { if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') { e.preventDefault(); const target = all[(i + (e.key === 'ArrowRight' ? 1 : all.length - 1)) % all.length]; switchView(target); document.getElementById(`tab-${target}`)?.focus(); } }}>{labels[v]}</button>)}</div>
      <div className="filterbar"><label className="search"><Search size={16} aria-hidden="true" /><input aria-label="Rechercher dans les résultats" type="search" value={query} onChange={e => setQuery(e.target.value)} /></label>
        {view === 'findings' && <label className="inline-label"><Filter size={16} aria-hidden="true" /><select aria-label="Type d’anomalie" value={kind} onChange={e => { setKind(e.target.value); setOffset(0); }}><option value="">Toutes les anomalies</option>{[...new Set([...Object.keys(lastSummary?.counts || {}), ...findings.map(f => f.kind)])].sort().map(k => <option key={k} value={k}>{k}</option>)}</select></label>}
        <label className="inline-label">Tri<select aria-label="Trier les résultats" value={sort} onChange={e => { setSort(e.target.value as typeof sort); setOffset(0); }}><option value="row">Ligne source</option><option value="kind">Type</option></select></label><label className="choice"><input type="checkbox" checked={descending} onChange={e => { setDescending(e.target.checked); setOffset(0); }} />Décroissant</label>
      </div>
      {view === 'findings' && page.items.length > 0 && <div className="batchbar"><label className="choice"><input type="checkbox" aria-label="Sélectionner les cellules de cette page" checked={keys.length > 0 && keys.every(key => selected.has(key))} disabled={loading || w.busy || w.edited} onChange={e => setSelected(e.target.checked ? new Set(keys) : new Set())} />{selected.size ? `${selected.size} cellule(s)` : 'Cette page'}</label><div className="actions"><Button disabled={!selected.size || w.busy || w.edited || loading} onClick={() => batch('keep')}>Conserver</Button><Button disabled={!selected.size || w.busy || w.edited || loading} onClick={() => batch('clear')}>Vider</Button><Button icon={Check} disabled={!selected.size || w.busy || w.edited || loading} onClick={() => batch('proposed')}>Accepter les propositions</Button></div></div>}
      <div id="result-panel" role="tabpanel" aria-labelledby={`tab-${view}`} aria-busy={loading}>
        {loading && <p role="status" className="loading-line">Chargement des résultats…</p>}
        {failed ? <Empty title="Résultats indisponibles"><Button icon={RefreshCw} onClick={() => setRetry(v => v + 1)}>Réessayer</Button></Empty> : !page.items.length && !loading ? <Empty title={canRead ? 'Aucun résultat pour cette vue' : 'Analyse requise'}>{search || kind ? <Button onClick={() => { setQuery(''); setSearch(''); setKind(''); }}>Réinitialiser les filtres</Button> : null}</Empty> : <div ref={scroll} className="results-scroll" tabIndex={0} aria-label={`Liste ${labels[view].toLowerCase()}`}>
          <div className="virtual-content" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map(virtual => {
              const item = page.items[virtual.index];
              return <div key={`${view}-${virtual.key}`} ref={virtualizer.measureElement} data-index={virtual.index} className="virtual-row" style={{ transform: `translateY(${virtual.start}px)` }}>
                {view === 'findings' ? (() => { const f = item as Finding; return <article className="finding" data-testid="finding"><header><label className="choice"><input type="checkbox" aria-label={`Sélectionner ${f.key}`} checked={selected.has(f.key)} disabled={w.busy || w.edited || loading} onChange={e => setSelected(old => { const next = new Set(old); if (e.target.checked) next.add(f.key); else next.delete(f.key); return next; })} /><strong>{f.message}</strong></label><span className={`status ${queued.has(f.key) ? '' : 'review'}`}>{queued.has(f.key) ? 'Décision en attente' : f.kind}</span></header><div className="anomaly-origin">{f.key} · {f.field_code}</div><div className="value-grid"><div className="value-box"><span>Source · {f.original.kind}</span><code>{f.original.value || '∅'}</code></div><div className="value-box"><span>Courante · {f.current.kind}</span><code>{f.current.value || '∅'}</code></div><div className="value-box"><span>Proposition</span><code>{f.proposed?.value || 'Décision manuelle requise'}</code></div></div><div className="actions end"><Button disabled={w.busy || w.edited || loading} onClick={() => w.act(() => submit({ [f.key]: decisionFor(f, 'keep', session.config.rules[f.field_code] || defaultRule(), 'Conservation explicite de la source') }))}>Conserver la source</Button><Button primary icon={Eye} disabled={w.busy || w.edited || loading} onClick={() => setEditing(f)}>Examiner et décider</Button></div></article>; })() : view === 'changes' ? (() => { const c = item as CellChange; return <article className="change"><strong className="mono">{c.key} · {c.field_code}</strong><div className="value-grid"><div className="value-box"><span>Avant · {c.before.kind}</span><code>{c.before.value || '∅'}</code></div><div className="value-box"><span>Après · {c.after.kind}</span><code>{c.after.value || '∅'}</code></div></div><p>{c.reason}</p></article>; })() : <HistoryRow entry={item as HistoryEntry} />}
              </div>;
            })}
          </div>
        </div>}
      </div>
      <Pager offset={offset} limit={limit} total={page.total} onPage={setOffset} onLimit={n => { setLimit(n); setOffset(0); }} disabled={loading || w.busy} />
    </Section>
    <footer className="stage-footer"><span className="field-hint">Les coordonnées sources restent inchangées après tri.</span><Button primary icon={ArrowRight} onClick={next} disabled={!current || w.busy}>Préparer l’export</Button></footer>
    {editing && <DecisionDialog finding={editing} onClose={() => setEditing(null)} onSave={(decision: Decision) => submit({ [editing.key]: decision })} />}
  </>;
}
function HistoryRow({ entry }: { entry: HistoryEntry }) {
  const value = (key: string) => typeof entry[key] === 'string' || typeof entry[key] === 'number' ? String(entry[key]) : '';
  return <article className="history-entry"><div className="anomaly-origin">{value('at') || value('timestamp') || value('date')}</div><strong>{value('message') || value('op') || value('action') || 'Événement du projet'}</strong>{value('key') && <p className="mono">{value('key')}</p>}<details><summary>Détails de l’événement</summary><pre>{JSON.stringify(entry, null, 2)}</pre></details></article>;
}
