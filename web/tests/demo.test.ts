import { beforeEach, describe, expect, it } from 'vitest';
import { DemoAdapter, DEMO_SOURCE } from '../src/services/demo';
import { cell, emptyCell, type Finding, type Op, type Operations, type Session } from '../src/model';

describe('browser synthetic adapter', () => {
  let adapter: DemoAdapter;
  let session: Session | undefined;
  const run = async <K extends Op,>(op: K, payload: Operations[K]['payload']) => adapter.request({ op, payload, task_id: crypto.randomUUID(), ...(session ? { session_id: session.id, revision: session.revision } : {}) }, () => {});
  const prepare = async () => {
    session = await run('demo', {});
    session = await run('configure', { config: { ...session.config, mappings: session.config.mappings.map(m => ({ ...m, confirmed: !!m.field_code })) } });
    session = await run('analyze', {});
  };
  beforeEach(() => { adapter = new DemoAdapter(0); session = undefined; });
  it('requires confirmation, pages results, and keeps original coordinates after sorting', async () => {
    session = await run('demo', {});
    expect(session.config.mappings.some(m => m.field_code && !m.confirmed)).toBe(true);
    await expect(run('analyze', {})).rejects.toMatchObject({ code: 'CONFIG_INVALID' });
    await prepare();
    const page = await run('results', { view: 'findings', offset: 25, limit: 25, sort: 'row', descending: true });
    expect(page.total).toBe(1200); expect(page.items).toHaveLength(25);
    const first = page.items[0] as Finding;
    expect(first.key).toBe(`${session!.config.sheet}!${first.row}:${first.column}`);
    expect(first.row).toBeGreaterThan((page.items.at(-1) as Finding).row);
  });
  it('batches decisions, preserves exact values, invalidates and supports undo/redo', async () => {
    await prepare();
    const key = `${session!.config.sheet}!2:2`;
    const exact = '12345678901234567890.123456789';
    session = await run('decide', { decisions: { [key]: { action: 'set', value: cell(exact, 'decimal'), reason: 'Test de précision', allow_multiple: false } } });
    expect(session.dirty).toBe(true); expect(session.can_undo).toBe(true);
    session = await run('analyze', {});
    expect(session.dirty).toBe(false); expect(session.pending_decisions).toBe(0);
    const changes = await run('results', { view: 'changes', offset: 0, limit: 50 });
    expect(changes.items[0]).toMatchObject({ key, after: { kind: 'decimal', value: exact } });
    session = await run('undo', {}); expect(session.dirty).toBe(true); expect(session.can_redo).toBe(true);
    session = await run('analyze', {}); expect(session.summary!.changes).toBe(0);
    session = await run('redo', {}); session = await run('analyze', {}); expect(session.summary!.changes).toBe(1);
    expect((await run('results', { view: 'history', offset: 0, limit: 50 })).total).toBeGreaterThan(3);
  });
  it('does not classify keep or required empty as resolved', async () => {
    await prepare();
    const sheet = session!.config.sheet;
    session = await run('decide', { decisions: { [`${sheet}!2:2`]: { action: 'keep', value: emptyCell(), allow_multiple: false, reason: 'Conserver' }, [`${sheet}!5:5`]: { action: 'clear', value: emptyCell(), allow_multiple: false, reason: 'Vider' } } });
    session = await run('analyze', {}); expect(session.summary!.findings).toBe(1200);
  });
  it('does not mark stale results current when saving a project', async () => {
    await prepare(); session = await run('configure', { config: { ...session!.config, header_row: 2 } });
    await run('project_save', { path: '/DEMO/projet-demo.fdxproj' });
    const opened = await run('project_open', { path: '/DEMO/projet-demo.fdxproj' });
    expect(opened.summary).toBeNull(); expect(opened.dirty).toBe(true);
  });
  it('saves late project details without invalidating the current analysis', async () => {
    await prepare();
    const summary = session!.summary;
    session = await run('metadata', { name: '  Suivi préparé  ', family: 'Passation' });
    expect(session.name).toBe('Suivi préparé'); expect(session.family).toBe('Passation');
    expect(session.dirty).toBe(false); expect(session.summary).toEqual(summary);
    await run('project_save', { path: '/DEMO/renomme.fdxproj' });
    const restored = await run('project_open', { path: '/DEMO/renomme.fdxproj' });
    expect(restored.name).toBe('Suivi préparé'); expect(restored.family).toBe('Passation');
  });
  it('preserves current analysis for report-only settings and resets profiles', async () => {
    await prepare();
    session = await run('configure', { config: { ...session!.config, harmonized: false, reports: [] } });
    expect(session.dirty).toBe(false); expect(session.summary).not.toBeNull();
    await run('profile_save', { path: '/DEMO/profil-demo.json' });
    session = await run('profile_load', { path: '/DEMO/profil-demo.json' });
    expect(session.config.mappings.every(m => !m.confirmed)).toBe(true);
  });
  it('rejects stale revisions, real file paths and Excel export', async () => {
    await prepare();
    await expect(adapter.request({ op: 'undo', payload: {}, task_id: crypto.randomUUID(), session_id: session!.id, revision: session!.revision - 1 }, () => {})).rejects.toMatchObject({ code: 'REVISION_CONFLICT' });
    await expect(run('inspect', { path: 'C:\\private\\source.xlsx', header_row: 1 })).rejects.toMatchObject({ code: 'DEMO_ONLY' });
    await expect(run('export', { path: '/DEMO/export.xlsx', allow_issues: true, overwrite: false })).rejects.toMatchObject({ code: 'DEMO_ONLY' });
    expect((await run('inspect', { path: DEMO_SOURCE, header_row: 1 })).path).toBe(DEMO_SOURCE);
  });
  it('cancels before committing changes', async () => {
    await prepare(); const task_id = crypto.randomUUID();
    const task = adapter.request({ op: 'analyze', payload: {}, task_id, session_id: session!.id, revision: session!.revision }, () => {});
    await adapter.cancel(task_id);
    await expect(task).rejects.toMatchObject({ code: 'CANCELLED' });
    session = await run('analyze', {}); expect(session.dirty).toBe(false);
  });
});
