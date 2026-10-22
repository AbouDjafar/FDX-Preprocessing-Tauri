import { useState } from 'react';
import { ArrowRight, CheckCheck, FileText, Pencil, Plus, Trash2 } from 'lucide-react';
import { columnName, newField, type FieldSpec } from '../model';
import { selectFile } from '../services/bridge';
import { useWorkspace } from '../workspace';
import { Button, Field, IconButton, Modal, Notice, Pager, Section, Toggle } from '../components/ui';

function FieldEditor({ field, codes, advanced, onSave, onClose }: { field: FieldSpec; codes: string[]; advanced: boolean; onSave: (field: FieldSpec) => void; onClose: () => void }) {
  const [value, setValue] = useState(structuredClone(field));
  const patch = (update: Partial<FieldSpec>) => setValue(v => ({ ...v, ...update }));
  const valid = !!value.code.trim() && !!value.label.trim() && !codes.includes(value.code.trim());
  return <Modal title={field.code ? `Champ · ${field.code}` : 'Nouveau champ'} onClose={onClose} wide>
    <form onSubmit={e => { e.preventDefault(); if (valid) onSave({ ...value, code: value.code.trim(), label: value.label.trim() }); }}>
      <div className="form-grid"><Field label="Code unique"><input autoFocus required value={value.code} onChange={e => patch({ code: e.target.value })} /></Field><Field label="Libellé"><input required value={value.label} onChange={e => patch({ label: e.target.value })} /></Field>
        <Field label="Type"><select value={value.dtype} onChange={e => patch({ dtype: e.target.value })}>{[['text', 'Texte'], ['integer', 'Entier'], ['decimal', 'Décimal'], ['date', 'Date'], ['boolean', 'Booléen']].map(([v, l]) => <option key={v} value={v}>{l}</option>)}</select></Field>
        <Field label="Rôle"><select value={value.role} onChange={e => patch({ role: e.target.value })}>{[['standard', 'Standard'], ['identifier', 'Identifiant'], ['phone', 'Téléphone'], ['company', 'Entreprise']].map(([v, l]) => <option key={v} value={v}>{l}</option>)}</select></Field>
        <Field label="Longueur maximale"><input type="number" min={1} value={value.max_length ?? ''} onChange={e => patch({ max_length: e.target.value ? Math.max(1, Math.trunc(Number(e.target.value))) : null })} /></Field>
        <Field label="Format de date"><input value={value.date_format} onChange={e => patch({ date_format: e.target.value })} /></Field>
      </div>
      <Toggle label="Champ obligatoire" checked={value.required} onChange={required => patch({ required })} />
      <details className="details" open={advanced || undefined}><summary>Contraintes avancées</summary><div className="form-grid">
        <Field label="Minimum (décimal exact)"><input inputMode="decimal" value={value.minimum ?? ''} onChange={e => patch({ minimum: e.target.value || null })} /></Field>
        <Field label="Maximum (décimal exact)"><input inputMode="decimal" value={value.maximum ?? ''} onChange={e => patch({ maximum: e.target.value || null })} /></Field>
        <Field label="Motif de validation"><input className="mono" value={value.pattern} onChange={e => patch({ pattern: e.target.value })} /></Field>
        <Field label="Valeurs autorisées (une par ligne)"><textarea rows={4} value={value.allowed_values.join('\n')} onChange={e => patch({ allowed_values: e.target.value.split('\n') })} /></Field>
      </div></details>
      {codes.includes(value.code.trim()) && <p className="validation" role="alert">Ce code est déjà utilisé.</p>}
      <footer className="modal-footer"><Button onClick={onClose}>Annuler</Button><Button primary type="submit" icon={CheckCheck} disabled={!valid}>Enregistrer le champ</Button></footer>
    </form>
  </Modal>;
}
export function MappingStage({ next, advanced }: { next: () => void; advanced: boolean }) {
  const w = useWorkspace(); const config = w.draft!;
  const [editor, setEditor] = useState<{ index: number; field: FieldSpec } | null>(null);
  const [paste, setPaste] = useState(false); const [text, setText] = useState('');
  const [mappingPage, setMappingPage] = useState(0); const [fieldPage, setFieldPage] = useState(0);
  const sheet = w.session!.workbook?.sheets.find(s => s.name === config.sheet);
  const columns = sheet?.columns || 0;
  const mappings = Array.from({ length: columns }, (_, i) => config.mappings.find(m => m.column === i + 1) || { column: i + 1, field_code: null, confirmed: false, score: 0 });
  const unconfirmed = mappings.filter(m => m.field_code && !m.confirmed).length;
  const updateMapping = (column: number, update: Partial<(typeof mappings)[number]>) => w.edit(c => ({ ...c, mappings: mappings.map(m => m.column === column ? { ...m, ...update } : m) }));
  const saveField = (field: FieldSpec) => {
    if (!editor) return;
    const old = config.fields[editor.index]?.code;
    w.edit(c => {
      const rules = { ...c.rules };
      if (old && old !== field.code && rules[old]) { rules[field.code] = rules[old]; delete rules[old]; }
      return { ...c, fields: editor.index < 0 ? [...c.fields, field] : c.fields.map((f, i) => i === editor.index ? field : f), rules,
        mappings: c.mappings.map(m => old && m.field_code === old ? { ...m, field_code: field.code, confirmed: false } : m),
        chronology: c.chronology.map(r => ({ ...r, before: old && r.before === old ? field.code : r.before, after: old && r.after === old ? field.code : r.after })) };
    }); setEditor(null);
  };
  const removeField = (index: number) => w.act(async () => {
    const code = config.fields[index].code;
    if (!await w.confirm('Supprimer ce champ ?', `${code} sera retiré du dictionnaire, des associations et des relations chronologiques. Les colonnes sources restent intactes.`, 'Supprimer le champ')) return;
    w.edit(c => { const rules = { ...c.rules }; delete rules[code]; return { ...c, rules, fields: c.fields.filter((_, i) => i !== index), mappings: c.mappings.map(m => m.field_code === code ? { ...m, field_code: null, confirmed: false } : m), chronology: c.chronology.filter(r => r.before !== code && r.after !== code) }; });
  });
  const loadDictionary = () => w.act(async () => {
    const path = await selectFile(w.mode, 'dictionary'); if (!path) return;
    if (!await w.confirm('Remplacer le dictionnaire ?', 'Les champs actuels seront remplacés et les associations proposées de nouveau.', 'Charger le dictionnaire')) return;
    await w.saveDraft(); await w.run('dictionary', { path });
  });
  return <>
    {w.session!.notes.length > 0 && <details className="details"><summary>Notes du dictionnaire ({w.session!.notes.length})</summary><ul>{w.session!.notes.map((note, i) => <li key={i}>{note}</li>)}</ul></details>}
    <Section title="2. Associations par position Excel" actions={<span className={`status ${unconfirmed ? 'review' : ''}`}>{unconfirmed} à confirmer</span>}>
      {!sheet ? <Notice warm>Importez une feuille avant d’associer ses colonnes.</Notice> : <>
        <div className="table-wrap"><table className="data-table"><thead><tr><th>Position</th><th>En-tête source</th><th>Champ métier</th><th>Confirmation</th></tr></thead><tbody>
          {mappings.slice(mappingPage, mappingPage + 25).map(m => <tr key={m.column}><th scope="row" className="mono">{columnName(m.column)} · {m.column}</th><td>{sheet.headers[m.column - 1]?.value || '(sans en-tête)'}<div className="smallmuted">{sheet.preview[0]?.[m.column - 1]?.value}</div></td><td><select aria-label={`Champ colonne ${columnName(m.column)}`} value={m.field_code || ''} onChange={e => updateMapping(m.column, { field_code: e.target.value || null, confirmed: false, score: 0 })}><option value="">Conserver sans traitement</option>{config.fields.map((f, i) => <option key={`${f.code}-${i}`} value={f.code} disabled={mappings.some(other => other.column !== m.column && other.field_code === f.code)}>{f.code} · {f.label}</option>)}</select></td><td><label className="choice"><input type="checkbox" checked={m.confirmed} disabled={!m.field_code} onChange={e => updateMapping(m.column, { confirmed: e.target.checked })} />Confirmée</label>{advanced && m.score > 0 && <span className="field-hint">Score proposé : {m.score}</span>}</td></tr>)}
        </tbody></table></div>
        <Pager offset={mappingPage} limit={25} total={columns} onPage={setMappingPage} />
        <div className="actions end"><Button icon={CheckCheck} disabled={!mappings.some(m => m.field_code)} onClick={() => w.act(async () => {
          if (await w.confirm('Confirmer les associations ?', `${mappings.filter(m => m.field_code).length} colonnes seront confirmées, y compris celles des autres pages.`, 'Confirmer toutes les associations')) w.edit(c => ({ ...c, mappings: mappings.map(m => ({ ...m, confirmed: !!m.field_code })) }));
        })}>Confirmer toutes les associations</Button></div>
      </>}
    </Section>
    <Section title={`Dictionnaire · ${config.fields.length} champs`} actions={<div className="actions"><Button icon={FileText} onClick={loadDictionary}>Importer</Button>{w.mode === 'desktop' && <Button onClick={() => setPaste(true)}>Coller un dictionnaire</Button>}<Button icon={Plus} onClick={() => setEditor({ index: -1, field: newField() })}>Ajouter un champ</Button></div>}>
      <div className="table-wrap"><table className="data-table"><thead><tr><th>Code</th><th>Libellé</th><th>Type / rôle</th><th>Contraintes</th><th>Actions</th></tr></thead><tbody>{config.fields.slice(fieldPage, fieldPage + 25).map((f, local) => <tr key={fieldPage + local}><th scope="row" className="mono">{f.code}</th><td>{f.label}</td><td>{f.dtype}<div className="smallmuted">{f.role}</div></td><td>{f.required ? 'Obligatoire' : 'Facultatif'}{f.max_length !== null && ` · ${f.max_length} caractères`}{f.minimum !== null && <div>Min. {f.minimum}</div>}{f.maximum !== null && <div>Max. {f.maximum}</div>}{f.allowed_values.length > 0 && <div>{f.allowed_values.length} valeurs autorisées</div>}</td><td><div className="actions"><IconButton icon={Pencil} label={`Modifier ${f.code}`} onClick={() => setEditor({ index: fieldPage + local, field: f })} /><IconButton icon={Trash2} label={`Supprimer ${f.code}`} onClick={() => removeField(fieldPage + local)} /></div></td></tr>)}</tbody></table></div>
      <Pager offset={fieldPage} limit={25} total={config.fields.length} onPage={setFieldPage} />
    </Section>
    <footer className="stage-footer"><span className="field-hint">Les colonnes non associées conservent leurs valeurs.</span><Button primary icon={ArrowRight} disabled={!mappings.some(m => m.field_code) || unconfirmed > 0} onClick={() => w.act(async () => { await w.saveDraft(); next(); })}>Enregistrer et définir les règles</Button></footer>
    {editor && <FieldEditor field={editor.field} codes={config.fields.filter((_, i) => i !== editor.index).map(f => f.code)} advanced={advanced} onSave={saveField} onClose={() => setEditor(null)} />}
    {paste && <Modal title="Dictionnaire métier" onClose={() => setPaste(false)} wide><Field label="Texte Markdown ou texte brut"><textarea rows={14} value={text} onChange={e => setText(e.target.value)} /></Field><footer className="modal-footer"><Button onClick={() => setPaste(false)}>Annuler</Button><Button primary disabled={!text.trim()} onClick={() => w.act(async () => { if (!await w.confirm('Remplacer le dictionnaire ?', 'Les champs et associations seront proposés de nouveau.', 'Importer le texte')) return; await w.saveDraft(); await w.run('dictionary', { text }); setPaste(false); })}>Importer le texte</Button></footer></Modal>}
  </>;
}
