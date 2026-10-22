import { isSupportedCountry, parsePhoneNumberFromString, type CountryCode } from 'libphonenumber-js/max';
import { cell, emptyCell, type Decision, type FieldRule, type Finding } from './model';

export type DecisionChoice = 'keep' | 'proposed' | 'clear' | 'manual' | 'first' | 'first_valid' | 'all';
export function firstValidPhone(finding: Finding, region: string) {
  if (!isSupportedCountry(region)) return undefined;
  return finding.choices.find(value => {
    try { return value.kind === 'text' && !!parsePhoneNumberFromString(value.value, region as CountryCode)?.isValid(); } catch { return false; }
  });
}
export function decisionFor(finding: Finding, choice: Exclude<DecisionChoice, 'manual'>, rule: FieldRule, reason: string): Decision {
  if (finding.original.kind === 'formula' && choice !== 'keep') throw { code: 'FORMULA_PROTECTED', message: 'Une formule ne peut pas être remplacée.' };
  const base: Decision = { action: 'set', value: emptyCell(), allow_multiple: false, reason };
  if (choice === 'keep') return { ...base, action: 'keep' };
  if (choice === 'clear') return { ...base, action: 'clear' };
  if (choice === 'all') {
    if (!finding.choices.length) throw { code: 'NO_CHOICES', message: 'Aucun numéro proposé.' };
    return { ...base, value: cell(finding.choices.map(v => v.value).join(rule.phone_separator)), allow_multiple: true };
  }
  const value = choice === 'proposed' ? finding.proposed : choice === 'first' ? finding.choices[0] : firstValidPhone(finding, rule.phone_region);
  if (!value) throw { code: 'NO_PROPOSAL', message: 'Aucune proposition disponible pour ce choix.' };
  return { ...base, value };
}
export function validManualValue(kind: string, value: string): boolean {
  if (kind === 'integer') return /^[+-]?\d+$/.test(value);
  if (kind === 'decimal') return /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(value);
  if (kind === 'boolean') return value === 'true' || value === 'false';
  if (kind === 'date') {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
    const date = new Date(`${value}T00:00:00Z`);
    return !Number.isNaN(date.getTime()) && date.toISOString().slice(0, 10) === value;
  }
  return kind === 'text';
}
