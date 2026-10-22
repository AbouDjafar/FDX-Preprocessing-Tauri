import { useState } from 'react';
import { ArrowRight, FolderOpen, Plus, Save, Trash2 } from 'lucide-react';
import { columnName, configIssues, defaultRule, type FieldRule, type SegmentationRule } from '../model';
import { selectFile } from '../services/bridge';
import { useWorkspace } from '../workspace';
import { Button, Field, IconButton, Modal, Notice, Section, Toggle } from '../components/ui';

function Replacements({ values, onChange }: { values: Record<string, string>; onChange: (values: Record<string, string>) => void }) {
  const [adding, setAdding] = useState(false); const [from, setFrom] = useState(''); const [to, setTo] = useState('');
  return <div><div className="section-head"><h3>Remplacements exacts</h3><Button icon={Plus} onClick={() => { setFrom(''); setTo(''); setAdding(true); }}>Ajouter</Button></div>
    {Object.entries(values).map(([source, target]) => <div className="replacement-row" key={source}><code>{source}</code><span aria-hidden="true">→</span><input aria-label={`Remplacement de ${source}`} value={target} onChange={e => onChange({ ...values, [source]: e.target.value })} /><IconButton icon={Trash2} label={`Retirer le remplacement de ${source}`} onClick={() => { const next = { ...values }; delete next[source]; onChange(next); }} /></div>)}
    {!Object.keys(values).length && <p className="field-hint">Aucun remplacement configuré.</p>}
    {adding && <Modal title="Remplacement exact" onClose={() => setAdding(false)}><Field label="Valeur source"><input autoFocus value={from} onChange={e => setFrom(e.target.value)} /></Field><Field label="Valeur de remplacement"><input value={to} onChange={e => setTo(e.target.value)} /></Field>{Object.hasOwn(values, from) && <p className="validation">Cette valeur source est déjà configurée.</p>}<footer className="modal-footer"><Button onClick={() => setAdding(false)}>Annuler</Button><Button primary disabled={!from || Object.hasOwn(values, from)} onClick={() => { onChange({ ...values, [from]: to }); setAdding(false); }}>Ajouter le remplacement</Button></footer></Modal>}
  </div>;
}
export function RulesStage({ next, advanced }: { next: () => void; advanced: boolean }) {
  const w = useWorkspace(); const config = w.draft!;
  const [selected, setSelected] = useState(config.fields[0]?.code || '');
  const field = config.fields.find(f => f.code === selected) || config.fields[0];
  const rule = (field && config.rules[field.code]) || defaultRule();
  const update = (patch: Partial<FieldRule>) => { if (field) w.edit(c => ({ ...c, rules: { ...c.rules, [field.code]: { ...(c.rules[field.code] || defaultRule()), ...patch } } })); };
  const sheet = w.session!.workbook?.sheets.find(s => s.name === config.sheet);
  const positions = Array.from({ length: sheet?.columns || 0 }, (_, i) => i + 1);
  const seg = config.segmentation;
  const segmentation = (patch: Partial<SegmentationRule>) => w.edit(c => ({ ...c, segmentation: { ...c.segmentation, ...patch } }));
  const issues = configIssues(config);
  const profile = (save: boolean) => w.act(async () => {
    const path = await selectFile(w.mode, 'profile', save, 'profil-fdx.json'); if (!path) return;
    if (save) { await w.saveDraft(); await w.run('profile_save', { path }); w.notify(w.mode === 'demo' ? 'Profil DEMO conservé en mémoire pour cette visite.' : 'Profil enregistré.'); }
    else {
      if (!await w.confirm('Charger ce profil ?', 'Les paramètres actuels seront remplacés. Les associations devront être confirmées pour ce classeur.', 'Charger le profil')) return;
      await w.run('profile_load', { path }); w.notify('Profil chargé. Reconfirmez les associations.');
    }
  });
  return <>
    <Section title="3. Règles de traitement" actions={<div className="actions"><Button icon={FolderOpen} onClick={() => profile(false)}>Charger un profil</Button><Button icon={Save} onClick={() => profile(true)}>Enregistrer le profil</Button></div>}>
      <Field label="Champ métier"><select value={field?.code || ''} onChange={e => setSelected(e.target.value)}>{config.fields.map(f => <option key={f.code} value={f.code}>{f.code} · {f.label}</option>)}</select></Field>
      {field && <div key={field.code}>
        <div className="form-grid"><Toggle label="Normaliser les espaces" checked={rule.normalize_spaces} onChange={normalize_spaces => update({ normalize_spaces })} /><Toggle label="Convertir selon le type confirmé" checked={rule.convert} onChange={convert => update({ convert })} /></div>
        {(advanced || ['integer', 'decimal'].includes(field.dtype)) && <fieldset className="rule-group"><legend>Nombres</legend><div className="form-grid">
          <Field label="Séparateur décimal"><select value={rule.decimal_separator} onChange={e => update({ decimal_separator: e.target.value })}><option value=",">Virgule (,)</option><option value=".">Point (.)</option></select></Field>
          <Field label="Séparateur de milliers"><select value={rule.thousands_separator} onChange={e => update({ thousands_separator: e.target.value })}>{[[' ', 'Espace'], ['', 'Aucun'], ['.', 'Point'], [',', 'Virgule'], ["'", 'Apostrophe'], ['\u00a0', 'Espace insécable'], ['\u202f', 'Espace fine insécable']].map(([v, label]) => <option key={v} value={v}>{label}</option>)}</select></Field>
          <Field label="Unités acceptées (une par ligne)"><textarea rows={3} value={rule.currency_tokens.join('\n')} onChange={e => update({ currency_tokens: e.target.value.split('\n') })} /></Field>
        </div>{rule.decimal_separator === rule.thousands_separator && <p className="validation">Les deux séparateurs doivent être différents.</p>}</fieldset>}
        {(advanced || field.dtype === 'date') && <fieldset className="rule-group"><legend>Dates</legend><Field label="Ordre de lecture"><select value={rule.date_order} onChange={e => update({ date_order: e.target.value })}><option value="dmy">Jour / mois / année</option><option value="mdy">Mois / jour / année</option><option value="ask">Demander pour chaque ambiguïté</option></select></Field></fieldset>}
        {(advanced || field.role === 'phone') && <fieldset className="rule-group"><legend>Téléphones</legend><div className="form-grid">
          <Field label="Région téléphonique (ISO)"><input maxLength={2} value={rule.phone_region} onChange={e => update({ phone_region: e.target.value.toUpperCase() })} /></Field>
          <Field label="Numéros multiples"><select value={rule.phone_policy} onChange={e => update({ phone_policy: e.target.value })}>{[['report', 'Conserver et signaler'], ['first', 'Premier numéro'], ['first_valid', 'Premier numéro valide'], ['all', 'Tous les numéros'], ['clear', 'Vider la cellule']].map(([v, label]) => <option key={v} value={v}>{label}</option>)}</select></Field>
          <Field label="Séparateur entre numéros"><input value={rule.phone_separator} onChange={e => update({ phone_separator: e.target.value })} /></Field>
        </div></fieldset>}
        <details className="details" open={advanced || undefined}><summary>Longueur, motif et remplacements</summary><div className="form-grid">
          <Field label="Dépassement de longueur"><select value={rule.length_policy} onChange={e => update({ length_policy: e.target.value })}><option value="report">Conserver et signaler</option><option value="truncate">Tronquer explicitement</option><option value="clear">Vider la cellule</option></select></Field>
          <Field label="Motif non respecté"><select value={rule.invalid_policy} onChange={e => update({ invalid_policy: e.target.value })}><option value="report">Conserver et signaler</option><option value="clear">Vider la cellule</option></select></Field>
        </div><Replacements values={rule.replacements} onChange={replacements => update({ replacements })} /></details>
        {(rule.length_policy !== 'report' || rule.invalid_policy === 'clear' || rule.phone_policy === 'clear') && <Notice warm>Une règle de troncature ou de vidage est active pour ce champ. Les valeurs originales resteront dans la source et les changements seront visibles avant export.</Notice>}
      </div>}
    </Section>
    <Section title="Segmentation des contacts">
      <Toggle label="Extraire les informations du contact" checked={seg.enabled} onChange={enabled => segmentation({ enabled })} />
      <fieldset disabled={!seg.enabled} className="unframed"><div className="form-grid"><Field label="Colonne source"><select value={seg.source_column} onChange={e => segmentation({ source_column: Number(e.target.value) })}>{positions.map(p => <option key={p} value={p}>{columnName(p)} · {sheet?.headers[p - 1]?.value || 'Sans en-tête'}</option>)}</select></Field>
        {([['name_column', 'Destination entreprise'], ['bp_column', 'Destination BP'], ['phone_column', 'Destination téléphone'], ['location_column', 'Destination localisation']] as const).map(([key, label]) => <Field key={key} label={label}><select value={seg[key] ?? ''} onChange={e => segmentation({ [key]: e.target.value ? Number(e.target.value) : null })}><option value="">Ne pas extraire</option>{positions.map(p => <option key={p} value={p} disabled={(['name_column', 'bp_column', 'phone_column', 'location_column'] as const).some(other => other !== key && seg[other] === p)}>{columnName(p)} · {sheet?.headers[p - 1]?.value || 'Sans en-tête'}</option>)}</select></Field>)}
      </div><Toggle label="Autoriser le remplacement de valeurs différentes aux destinations" checked={seg.overwrite} onChange={overwrite => segmentation({ overwrite })} /></fieldset>
    </Section>
    <Section title="Relations chronologiques" actions={<Button icon={Plus} onClick={() => w.edit(c => ({ ...c, chronology: [...c.chronology, { before: '', after: '', enabled: true }] }))}>Ajouter une relation</Button>}>
      {config.chronology.length === 0 && <p className="field-hint">Aucune relation chronologique configurée.</p>}
      {config.chronology.map((relation, i) => <div key={i} className="chronology-row"><label className="choice"><input aria-label={`Activer la relation ${i + 1}`} type="checkbox" checked={relation.enabled} onChange={e => w.edit(c => ({ ...c, chronology: c.chronology.map((r, n) => n === i ? { ...r, enabled: e.target.checked } : r) }))} />Active</label>
        {(['before', 'after'] as const).map(key => <Field key={key} label={key === 'before' ? 'Date antérieure ou égale' : 'Date postérieure ou égale'}><select value={relation[key]} onChange={e => w.edit(c => ({ ...c, chronology: c.chronology.map((r, n) => n === i ? { ...r, [key]: e.target.value } : r) }))}><option value="">Choisir un champ</option>{config.fields.filter(f => f.dtype === 'date').map(f => <option value={f.code} key={f.code}>{f.code} · {f.label}</option>)}</select></Field>)}
        <IconButton icon={Trash2} label={`Retirer la relation ${i + 1}`} onClick={() => w.edit(c => ({ ...c, chronology: c.chronology.filter((_, n) => n !== i) }))} />
      </div>)}
    </Section>
    {issues.length > 0 && <Notice warm><strong>Préparation à compléter</strong><ul>{issues.map(issue => <li key={issue}>{issue}</li>)}</ul></Notice>}
    <footer className="stage-footer"><span className="field-hint">Les décisions individuelles priment sur les règles générales.</span><Button primary icon={ArrowRight} disabled={issues.length > 0 || !field} onClick={() => w.act(async () => { await w.analyze(); next(); })}>Confirmer et analyser toutes les lignes</Button></footer>
  </>;
}
