import { describe, expect, it } from 'vitest';
import { analysisConfig, cell, columnName, configIssues, defaultConfig, defaultRule, emptyCell, filename, newField, normalizeError } from '../src/model';
import { decisionFor, firstValidPhone, validManualValue } from '../src/decisions';
import type { Finding } from '../src/model';

const finding = (patch: Partial<Finding> = {}): Finding => ({ key: 'Feuille!18:3', row: 18, column: 3, field_code: 'MONTANT', kind: 'number', message: 'À vérifier', original: cell('8.450.000'), current: cell('8.450.000'), proposed: cell('8450000', 'decimal'), choices: [], ...patch });
describe('Rust wire model', () => {
  it('matches version 2 and safe defaults', () => {
    expect(defaultConfig()).toMatchObject({ version: 2, header_row: 1, harmonized: true, report_collision: 'suffix', reports: ['control', 'original', 'verify'] });
    expect(defaultRule()).toMatchObject({ date_order: 'dmy', phone_region: 'CM', phone_policy: 'report', invalid_policy: 'report', length_policy: 'report' });
    expect(newField()).toMatchObject({ minimum: null, maximum: null, role: 'standard', dtype: 'text' });
    expect(emptyCell()).toEqual({ kind: 'empty', value: '' });
  });
  it('preserves arbitrary decimal strings and numeric kinds', () => {
    const value = '123456789012345678901234567890.1234567890123456789';
    const decision = decisionFor(finding({ proposed: cell(value, 'decimal') }), 'proposed', defaultRule(), 'Vérifié');
    expect(JSON.parse(JSON.stringify(decision)).value).toEqual({ kind: 'decimal', value });
    expect(validManualValue('decimal', value)).toBe(true);
    expect(validManualValue('integer', '5.5')).toBe(false);
    expect(validManualValue('decimal', '5,5')).toBe(false);
    expect(validManualValue('date', '2025-02-31')).toBe(false);
  });
  it('ignores output-only settings in analysis comparison', () => {
    const config = defaultConfig();
    expect(analysisConfig(config)).toBe(analysisConfig({ ...config, harmonized: false, reports: [], report_collision: 'replace' }));
    expect(analysisConfig(config)).not.toBe(analysisConfig({ ...config, header_row: 2 }));
  });
  it('identifies Excel positions, not duplicate header labels', () => {
    expect([1, 26, 27, 702, 703, 16384].map(columnName)).toEqual(['A', 'Z', 'AA', 'ZZ', 'AAA', 'XFD']);
    expect(filename('C:\\synthetic\\DEMO.xlsx')).toBe('DEMO.xlsx');
    const config = { ...defaultConfig(), source_path: '/DEMO/a.xlsx', sheet: 'Feuille', fields: [{ ...newField(), code: 'A', label: 'A' }], mappings: [{ column: 1, field_code: 'A', confirmed: false, score: 0 }] };
    expect(configIssues(config)).toContain('Confirmez chaque association active.');
    expect(configIssues({ ...config, mappings: config.mappings.map(m => ({ ...m, confirmed: true })) })).toEqual([]);
  });
  it('keeps original errors in details when not a French structured error', () => {
    expect(normalizeError({ code: 'LOCKED', message: 'Fichier verrouillé.', details: { path: '/DEMO/test.xlsx' } }).code).toBe('LOCKED');
    expect(normalizeError(new Error('technical detail')).details).toBe('technical detail');
  });
});
describe('explicit decisions', () => {
  it('does not invent a missing proposal or replace a formula', () => {
    expect(() => decisionFor(finding({ proposed: null }), 'proposed', defaultRule(), 'Motif')).toThrow();
    expect(() => decisionFor(finding({ original: cell('=SUM(A1:A3)', 'formula') }), 'clear', defaultRule(), 'Motif')).toThrow();
    expect(decisionFor(finding(), 'clear', defaultRule(), 'Vider')).toMatchObject({ action: 'clear', value: emptyCell() });
    expect(decisionFor(finding(), 'keep', defaultRule(), 'Conserver')).toMatchObject({ action: 'keep', allow_multiple: false });
  });
  it('supports first, first valid, chosen and all phone values without number coercion', () => {
    const phone = finding({ kind: 'phone_multiple', choices: [cell('699'), cell('+33612345678'), cell('+33687654321')] });
    const rule = { ...defaultRule(), phone_region: 'FR', phone_separator: ' / ' };
    expect(firstValidPhone(phone, 'FR')?.value).toBe('+33612345678');
    expect(firstValidPhone(phone, 'ZZ')).toBeUndefined();
    expect(decisionFor(phone, 'first', rule, 'Choix').value.value).toBe('699');
    expect(decisionFor(phone, 'first_valid', rule, 'Choix').value.value).toBe('+33612345678');
    expect(decisionFor(phone, 'all', rule, 'Choix')).toMatchObject({ action: 'set', allow_multiple: true, value: { kind: 'text', value: '699 / +33612345678 / +33687654321' } });
  });
});
