import { useState } from 'react';
import { Check } from 'lucide-react';
import { cell, defaultRule, type Decision, type Finding } from '../model';
import { decisionFor, firstValidPhone, validManualValue, type DecisionChoice } from '../decisions';
import { useWorkspace } from '../workspace';
import { Button, Field, Modal, Notice } from './ui';

export function DecisionDialog({ finding, onClose, onSave }: { finding: Finding; onClose: () => void; onSave: (decision: Decision) => Promise<void> }) {
  const w = useWorkspace(); const config = w.draft!;
  const field = config.fields.find(f => f.code === finding.field_code);
  const rule = config.rules[finding.field_code] || defaultRule();
  const phone = field?.role === 'phone'; const formula = finding.original.kind === 'formula';
  const [choice, setChoice] = useState<DecisionChoice | 'choice'>(formula ? 'keep' : finding.proposed ? 'proposed' : 'manual');
  const [index, setIndex] = useState(0);
  const [kind, setKind] = useState(field?.role === 'identifier' || phone ? 'text' : field?.dtype || 'text');
  const [value, setValue] = useState(finding.proposed?.value || finding.current.value);
  const [reason, setReason] = useState('Décision utilisateur');
  const [allowMultiple, setAllowMultiple] = useState(false);
  const valid = reason.trim() && (choice !== 'manual' || validManualValue(kind, value)) && (choice !== 'choice' || !!finding.choices[index]);
  const submit = () => w.act(async () => {
    const decision: Decision = choice === 'manual' ? { action: 'set', value: cell(value, kind), allow_multiple: phone && allowMultiple, reason } : choice === 'choice' ? { action: 'set', value: finding.choices[index], allow_multiple: false, reason } : decisionFor(finding, choice, rule, reason);
    if (phone && choice === 'keep') decision.allow_multiple = allowMultiple;
    await onSave(decision); onClose();
  });
  return <Modal title={`Décision · ${finding.key}`} onClose={onClose} wide>
    <p>{finding.message}</p><div className="value-grid"><div className="value-box"><span>Source · {finding.original.kind}</span><code>{finding.original.value || '∅'}</code></div><div className="value-box"><span>Valeur courante · {finding.current.kind}</span><code>{finding.current.value || '∅'}</code></div></div>
    {formula && <Notice warm>Formule protégée. Seule la conservation de la source est autorisée.</Notice>}
    <Field label="Décision"><select value={choice} onChange={e => setChoice(e.target.value as typeof choice)}>
      <option value="keep">Conserver la source et signaler</option>
      {!formula && <><option value="proposed" disabled={!finding.proposed}>Accepter la proposition</option><option value="choice" disabled={!finding.choices.length}>Choisir une valeur proposée</option>{phone && <><option value="first" disabled={!finding.choices.length}>Premier numéro</option><option value="first_valid" disabled={!firstValidPhone(finding, rule.phone_region)}>Premier numéro valide</option><option value="all" disabled={!finding.choices.length}>Tous les numéros</option></>}<option value="clear">Vider la cellule</option><option value="manual">Saisie manuelle</option></>}
    </select></Field>
    {choice === 'proposed' && <div className="value-box"><span>Proposition · {finding.proposed?.kind}</span><code>{finding.proposed?.value}</code></div>}
    {choice === 'choice' && <Field label="Valeur proposée"><select value={index} onChange={e => setIndex(Number(e.target.value))}>{finding.choices.map((v, i) => <option key={i} value={i}>{v.value} · {v.kind}</option>)}</select></Field>}
    {['first', 'first_valid', 'all'].includes(choice) && <div className="value-box"><span>Valeur retenue</span><code>{choice === 'all' ? finding.choices.map(v => v.value).join(rule.phone_separator) : choice === 'first_valid' ? firstValidPhone(finding, rule.phone_region)?.value : finding.choices[0]?.value}</code></div>}
    {choice === 'manual' && <div className="form-grid"><Field label="Type de la valeur"><select value={kind} onChange={e => setKind(e.target.value)} disabled={phone || field?.role === 'identifier'}>{[['text', 'Texte'], ['integer', 'Entier'], ['decimal', 'Décimal exact'], ['date', 'Date ISO'], ['boolean', 'Booléen']].map(([v, label]) => <option key={v} value={v}>{label}</option>)}</select></Field><Field label="Valeur manuelle" hint={kind === 'decimal' ? 'Point décimal, sans groupement. Précision conservée.' : undefined}>{kind === 'boolean' ? <select value={value} onChange={e => setValue(e.target.value)}><option value="">Choisir</option><option value="true">Oui</option><option value="false">Non</option></select> : <input type={kind === 'date' ? 'date' : 'text'} inputMode={['decimal', 'integer'].includes(kind) ? 'decimal' : 'text'} value={value} onChange={e => setValue(e.target.value)} />}</Field></div>}
    {phone && ['keep', 'manual'].includes(choice) && <label className="choice"><input type="checkbox" checked={allowMultiple} onChange={e => setAllowMultiple(e.target.checked)} />Autoriser explicitement plusieurs numéros</label>}
    {choice === 'clear' && <Notice warm>La cellule sera vidée, jamais la ligne. Une contrainte obligatoire peut rester en anomalie.</Notice>}
    <Field label="Motif de la décision"><textarea rows={2} value={reason} onChange={e => setReason(e.target.value)} /></Field>
    <footer className="modal-footer"><Button disabled={w.busy} onClick={onClose}>Annuler</Button><Button primary icon={Check} disabled={!valid || w.busy} onClick={submit}>Ajouter la décision au lot</Button></footer>
  </Modal>;
}
