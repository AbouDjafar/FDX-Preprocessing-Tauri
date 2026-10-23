import { useState } from 'react';
import { ArrowLeft, Download, FileCheck2, FolderOpen, RefreshCw, ShieldCheck } from 'lucide-react';
import { analysisConfig, filename } from '../model';
import { selectFile } from '../services/bridge';
import { useWorkspace } from '../workspace';
import { Button, Field, Notice, Section, Toggle } from '../components/ui';

export function ExportStage({ back }: { back: () => void }) {
  const w = useWorkspace(); const session = w.session!; const config = w.draft!;
  const [allowIssues, setAllowIssues] = useState(false); const [overwrite, setOverwrite] = useState(false);
  const [receipt, setReceipt] = useState(false);
  const current = !!session.summary && !session.dirty && analysisConfig(config) === analysisConfig(session.config);
  const ready = current && (!session.summary!.findings || allowIssues);
  const reports = [['control', 'Contrôle', 'Dictionnaire et indicateurs des champs associés'], ['original', 'Nom_original', 'Valeurs sources des cellules modifiées et segmentées'], ['verify', 'À vérifier', 'Anomalies restantes du résultat final']];
  const exportFile = () => w.act(async () => {
    if (!ready) return;
    await w.saveDraft();
    const path = await selectFile(w.mode, 'export', true, `${filename(config.original_path || config.source_path).replace(/\.xlsx$/i, '')}_fiabilise.xlsx`);
    if (!path) return;
    if (!await w.confirm('Exporter le classeur ?', `${filename(path)} · ${session.summary!.findings} anomalie(s) conservée(s). ${overwrite ? 'Vous autorisez explicitement le remplacement du fichier de destination s’il existe.' : 'Un fichier de destination existant ne sera pas remplacé.'}`, 'Exporter et vérifier')) return;
    await w.run('export', { path, allow_issues: allowIssues, overwrite }); w.notify('Export terminé et vérifié par le moteur.');
  });
  const downloadReceipt = () => w.act(async () => {
    if (!ready) return;
    await w.saveDraft();
    const blob = new Blob([JSON.stringify({ mode: 'DEMO', real_excel_export: false, description: 'Relevé du scénario synthétique uniquement. Aucun classeur produit.', summary: session.summary, reports: config.harmonized ? config.reports : [], allow_issues: allowIssues }, null, 2)], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob); const link = document.createElement('a'); link.href = url; link.download = 'FDX-DEMO-releve-non-excel.json'; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1500);
    setReceipt(true); w.notify('Relevé DEMO téléchargé. Aucun export Excel réel.');
  });
  return <>
    {w.mode === 'demo' && <Notice warm><strong>DEMO · aucun export Excel réel.</strong> Seul un relevé JSON du scénario synthétique peut être téléchargé. Les fichiers de production sont réservés à l’application de bureau.</Notice>}
    {!current && <Notice warm><strong>Analyse à actualiser.</strong> Les paramètres ou décisions ont changé. <Button icon={RefreshCw} disabled={w.busy} onClick={() => w.act(w.analyze)}>Vérifier avant export</Button></Notice>}
    <Section title="5. Préparer le reporting">
      <div className="grid-two"><div>
        <Toggle label="Inclure les rapports harmonisés" checked={config.harmonized} onChange={harmonized => w.edit(c => ({ ...c, harmonized }))} />
        <fieldset className="unframed" disabled={!config.harmonized}>{reports.map(([code, label, hint]) => <Toggle key={code} label={label} hint={hint} checked={config.reports.includes(code)} onChange={checked => w.edit(c => ({ ...c, reports: checked ? [...new Set([...c.reports, code])] : c.reports.filter(r => r !== code) }))} />)}
          <Field label="Rapports de même nom"><select value={config.report_collision} onChange={e => { const value = e.target.value; w.act(async () => { if (value === 'replace' && !await w.confirm('Remplacer les anciens rapports ?', 'Les feuilles portant les noms des rapports pourront être remplacées dans la copie exportée.', 'Autoriser le remplacement')) return; w.edit(c => ({ ...c, report_collision: value })); }); }}><option value="suffix">Ajouter un suffixe unique</option><option value="replace">Remplacer explicitement</option></select></Field>
        </fieldset>
      </div><div className="export-summary">
        <h3>État de la vérification</h3><dl><div><dt>Source</dt><dd className="mono">{filename(config.original_path || config.source_path)}</dd></div><div><dt>Feuille</dt><dd>{config.sheet}</dd></div><div><dt>Lignes analysées</dt><dd>{session.summary?.row_count.toLocaleString('fr-FR') ?? '—'}</dd></div><div><dt>Changements</dt><dd>{session.summary?.changes ?? '—'}</dd></div><div><dt>Anomalies restantes</dt><dd>{session.summary?.findings ?? '—'}</dd></div><div><dt>Résultat</dt><dd className={`status ${current ? '' : 'review'}`}>{current ? 'À jour' : 'Périmé ou absent'}</dd></div></dl>
        <label className="choice consent"><input type="checkbox" checked={allowIssues} onChange={e => setAllowIssues(e.target.checked)} />J’autorise l’export avec les anomalies restantes, sans les déclarer conformes.</label>
        {w.mode === 'desktop' && <label className="choice consent"><input type="checkbox" checked={overwrite} onChange={e => setOverwrite(e.target.checked)} />J’autorise le remplacement du fichier de destination s’il existe.</label>}
      </div></div>
    </Section>
    <footer className="stage-footer"><Button icon={ArrowLeft} onClick={back}>Revoir les résultats</Button>{w.mode === 'desktop' ? <Button data-testid="export-button" primary icon={FileCheck2} disabled={!ready || w.busy} onClick={exportFile}>Choisir la destination et exporter</Button> : <Button data-testid="demo-receipt" primary icon={Download} disabled={!ready || w.busy} onClick={downloadReceipt}>Télécharger le relevé DEMO (JSON)</Button>}</footer>
    {session.export_path && w.mode === 'desktop' && <Section title="Export vérifié" actions={<ShieldCheck size={22} />}><p className="mono">{filename(session.export_path)}</p><div className="actions"><Button icon={FileCheck2} onClick={() => w.act(() => w.run('open_path', { path: session.export_path! }))}>Ouvrir le classeur</Button><Button icon={FolderOpen} onClick={() => w.act(() => w.run('open_path', { path: session.export_path!, folder: true }))}>Ouvrir le dossier</Button></div></Section>}
    {receipt && <Notice>Relevé DEMO généré. Aucun fichier .xlsx n’a été créé ni vérifié.</Notice>}
  </>;
}
