import { useEffect, useId, useRef, useState } from 'react';
import { ArrowRight, Eye, EyeOff, FileSpreadsheet, FileText, FolderOpen, RefreshCw, ShieldCheck } from 'lucide-react';
import { columnName, filename, workbookReadError, type WorkbookInfo } from '../model';
import { selectFile, subscribeDrop } from '../services/bridge';
import { useWorkspace } from '../workspace';
import { Button, Field, Notice, Section } from '../components/ui';

const sourcePattern = /\.(xlsx|xltx|xlsm|xltm|xls|xlsb|xlt|xla|xlam)$/i;

export function ImportStage({ next }: { next: () => void }) {
  const w = useWorkspace();
  const session = w.session!;
  const pending = w.importDraft?.sessionId === session.id ? w.importDraft : null;
  const [info, setInfo] = useState<WorkbookInfo | null>(pending?.info ?? session.workbook);
  const [path, setPath] = useState(pending?.path ?? session.config.source_path);
  const [dictionary, setDictionary] = useState(pending?.dictionary ?? session.config.dictionary_path);
  const [sheet, setSheet] = useState(pending?.sheet ?? session.config.sheet);
  const [header, setHeader] = useState(pending?.header ?? session.config.header_row);
  const [inspectedHeader, setInspectedHeader] = useState(pending?.inspectedHeader ?? session.config.header_row);
  const [hovering, setHovering] = useState(false);
  const [previewVisible, setPreviewVisible] = useState(false);
  const previewId = useId();
  const selected = info?.sheets.find(s => s.name === sheet);
  const sourceExtension = /\.[^./\\]+$/.exec(path)?.[0].toLowerCase();
  const previewReady = !!selected && header === inspectedHeader && header <= selected.rows;
  useEffect(() => { w.updateImportDraft({ sessionId: session.id, info, path, dictionary, sheet, header, inspectedHeader }); }, [session.id, info, path, dictionary, sheet, header, inspectedHeader, w.updateImportDraft]);
  useEffect(() => { setPreviewVisible(false); }, [path, sheet, header]);
  const inspect = async (source: string, row = header) => {
    setPreviewVisible(false);
    // The native engine validates the content, not just the file extension.
    const result = await w.run('inspect', { path: source, header_row: row });
    setPath(source); setInfo(result); setInspectedHeader(row);
    setSheet(result.sheets.some(s => s.name === sheet) ? sheet : result.suggested_sheet);
  };
  const dropHandler = useRef((paths: string[]) => {});
  dropHandler.current = paths => {
    if (w.busy) return;
    w.act(async () => {
      const sources = paths.filter(p => sourcePattern.test(p));
      const dictionaries = paths.filter(p => /\.(md|txt)$/i.test(p));
      if (sources.length + dictionaries.length !== paths.length) throw workbookReadError({ code: 'SOURCE_TYPE', message: 'Ce type de fichier n’est pas pris en charge. Choisissez un classeur .xlsx ou un modèle .xltx ; renommer son extension ne le convertit pas.' });
      if (sources.length > 1 || dictionaries.length > 1) throw { code: 'DROP_FILES', message: 'Déposez un seul fichier Excel et, éventuellement, un dictionnaire .md ou .txt. Les formats importables sont .xlsx et .xltx.' };
      if (dictionaries[0]) setDictionary(dictionaries[0]);
      if (sources[0]) await inspect(sources[0]);
    });
  };
  useEffect(() => {
    if (w.mode !== 'desktop') return;
    let disposed = false; let unlisten: (() => void) | undefined;
    void subscribeDrop(paths => dropHandler.current(paths), value => { if (!disposed) setHovering(value); }).then(fn => { if (disposed) fn(); else unlisten = fn; }).catch(w.reportError);
    return () => { disposed = true; unlisten?.(); };
  }, [w.mode, w.reportError]);
  const choose = (kind: 'source' | 'dictionary') => w.act(async () => {
    const chosen = await selectFile(w.mode, kind); if (!chosen) return;
    if (kind === 'source') await inspect(chosen); else setDictionary(chosen);
  });
  const commit = () => w.act(async () => {
    if (session.pending_decisions || session.summary || w.edited) {
      if (!await w.confirm('Réimporter la source ?', 'Les décisions et résultats du projet actif seront effacés. Le classeur original reste intact.', 'Réimporter')) return;
    }
    await w.run('import', { path, ...(dictionary ? { dictionary_path: dictionary } : {}), sheet, header_row: header }); next();
  });
  return <>
    {w.mode === 'demo' && <Notice warm><strong>DEMO · données synthétiques uniquement.</strong> Aucun fichier local n’est lu. Les chemins /DEMO/ sont fictifs ; aucun export Excel réel n’est possible.</Notice>}
    <Section title="1. Sources et structure">
      <div className="grid-two">
        <div>
          <div className={`dropzone ${hovering ? 'drag-over' : ''}`}>
            <FileSpreadsheet size={28} aria-hidden="true" /><div className="grow"><strong>{filename(path) || 'Classeur de suivi'}</strong><p>{w.mode === 'desktop' ? `${path ? `${sourceExtension === '.xltx' ? 'Modèle' : 'Classeur'} ${sourceExtension || 'Excel'}` : 'Classeur .xlsx ou modèle .xltx'} · original préservé` : '1 200 lignes fictives · fixture intégrée'}</p></div>
            <Button icon={FolderOpen} disabled={w.busy} onClick={() => choose('source')}>{w.mode === 'demo' ? 'Choisir l’exemple' : 'Parcourir'}</Button>
          </div>
          <div className="dropzone"><FileText size={26} aria-hidden="true" /><div className="grow"><strong>{filename(dictionary) || 'Dictionnaire métier'}</strong><p>.md / .txt · facultatif à l’import</p></div><Button icon={FolderOpen} disabled={w.busy} onClick={() => choose('dictionary')}>Dictionnaire</Button></div>
          {dictionary && <Button onClick={() => setDictionary('')} disabled={w.busy}>Retirer le dictionnaire sélectionné</Button>}
        </div>
        <div>
          <Field label="Feuille de calcul"><select value={sheet} onChange={e => { setPreviewVisible(false); setSheet(e.target.value); }} disabled={!info || w.busy}><option value="">Choisir une feuille</option>{info?.sheets.map(s => <option key={s.name} value={s.name}>{s.name} · {s.rows} lignes</option>)}</select></Field>
          <Field label="Ligne d’en-tête"><input type="number" min={1} max={selected?.rows || 1048576} value={header} disabled={w.busy} onChange={e => { setPreviewVisible(false); setHeader(Math.max(1, Math.trunc(Number(e.target.value)) || 1)); }} /></Field>
          {selected && selected.suggested_header !== header && <Button disabled={w.busy} onClick={() => { setHeader(selected.suggested_header); w.act(() => inspect(path, selected.suggested_header)); }}>Utiliser la ligne suggérée ({selected.suggested_header})</Button>}
          <div className="actions"><Button icon={RefreshCw} disabled={!path || w.busy} onClick={() => w.act(() => inspect(path))}>Actualiser l’aperçu</Button><Button icon={previewVisible ? EyeOff : Eye} aria-expanded={previewVisible && previewReady} aria-controls={previewId} disabled={!previewReady || w.busy} onClick={() => setPreviewVisible(value => !value)}>{previewVisible ? 'Masquer l’aperçu' : 'Voir l’aperçu'}</Button></div>
          {header !== inspectedHeader && <p className="validation">Actualisez l’aperçu pour confirmer la nouvelle ligne d’en-tête.</p>}
        </div>
      </div>
    </Section>
    <div id={previewId} hidden={!previewVisible || !previewReady}>{previewVisible && previewReady && selected && <Section title={`Aperçu · ${selected.name}`} actions={<span className="mono">{selected.rows.toLocaleString('fr-FR')} lignes · {selected.columns} colonnes</span>}>
      <div className="table-wrap"><table className="data-table"><caption className="sr-only">Aperçu de la feuille sélectionnée, avec positions Excel</caption><thead><tr><th scope="col">Position</th>{Array.from({ length: selected.columns }, (_, i) => <th key={i} scope="col"><span className="mono">{columnName(i + 1)} · {i + 1}</span><br />{selected.headers[i]?.value || '(sans en-tête)'}</th>)}</tr></thead><tbody>{selected.preview.map((row, i) => <tr key={i}><th scope="row">Aperçu {i + 1}</th>{Array.from({ length: selected.columns }, (_, c) => <td key={c}><span className="cell-text">{row[c]?.value || '∅'}</span><small className="field-hint">{row[c]?.kind}</small></td>)}</tr>)}</tbody></table></div>
      <p className="field-hint">Échantillon de prévisualisation. L’analyse portera sur toutes les lignes après l’en-tête.</p>
    </Section>}</div>
    <footer className="stage-footer"><span className="subtle"><ShieldCheck size={16} /> Source originale intacte</span><Button primary icon={ArrowRight} disabled={!path || !selected || w.busy || header !== inspectedHeader || header > selected.rows} onClick={commit}>Importer et associer</Button></footer>
  </>;
}
