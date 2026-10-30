import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Progress } from '../src/model';

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), isTauri: vi.fn(() => false), open: vi.fn(), save: vi.fn(), listen: vi.fn(), close: vi.fn(), closeListen: vi.fn(), channel: null as null | { onmessage: (p: Progress) => void } }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke, isTauri: mocks.isTauri, Channel: class { onmessage = (_p: Progress) => {}; constructor() { mocks.channel = this; } } }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: mocks.open, save: mocks.save }));
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ onDragDropEvent: mocks.listen }) }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ destroy: mocks.close, onCloseRequested: mocks.closeListen }) }));
import { createAdapter, DesktopAdapter, selectFile, subscribeClose, subscribeDrop } from '../src/services/bridge';

describe('native contract', () => {
  beforeEach(() => { vi.clearAllMocks(); mocks.isTauri.mockReturnValue(false); });
  it('detects desktop using the core API, without invoking it in a browser', () => {
    expect(createAdapter().mode).toBe('demo'); expect(mocks.invoke).not.toHaveBeenCalled();
    mocks.isTauri.mockReturnValue(true); expect(createAdapter().mode).toBe('desktop');
  });
  it('sends the exact wrapper and channels only matching task events', async () => {
    const events: Progress[] = [];
    const request = { op: 'decide' as const, session_id: 'session', revision: 8, task_id: crypto.randomUUID(), payload: { decisions: { 'Sheet!3:4': { action: 'set', value: { kind: 'decimal', value: '9007199254740993.125' }, allow_multiple: false, reason: 'Confirmé' } } } };
    mocks.invoke.mockImplementation(async () => {
      mocks.channel!.onmessage({ task_id: 'other', phase: 'analyze', current: 0, total: null, message: 'Ignoré' });
      mocks.channel!.onmessage({ task_id: request.task_id, phase: 'decide', current: 1, total: 1, message: 'Terminé' });
      return { id: 'session', revision: 9 };
    });
    await new DesktopAdapter().request(request, e => events.push(e));
    expect(mocks.invoke).toHaveBeenCalledWith('fdx', { request, onEvent: mocks.channel });
    expect(events).toHaveLength(1);
    mocks.channel!.onmessage({ task_id: request.task_id, phase: 'decide', current: 1, total: 1, message: 'Late' }); expect(events).toHaveLength(1);
  });
  it('uses taskId for cancellation and never falls back to fixtures on native failure', async () => {
    mocks.isTauri.mockReturnValue(true); mocks.invoke.mockRejectedValue({ code: 'NATIVE_FAILED', message: 'Moteur indisponible.' });
    const adapter = createAdapter(); await expect(adapter.request({ op: 'demo', task_id: crypto.randomUUID(), payload: {} }, () => {})).rejects.toMatchObject({ code: 'NATIVE_FAILED' });
    mocks.invoke.mockResolvedValue(undefined); await adapter.cancel('task-id'); expect(mocks.invoke).toHaveBeenLastCalledWith('cancel_task', { taskId: 'task-id' });
  });
  it('uses native dialogs without browser fakepath or filesystem permissions', async () => {
    mocks.open.mockResolvedValue('C:\\Synthetic\\source.xlsx');
    expect(await selectFile('desktop', 'source')).toBe('C:\\Synthetic\\source.xlsx');
    expect(mocks.open).toHaveBeenCalledWith(expect.objectContaining({ multiple: false, directory: false, filters: [{ name: 'Fichiers Excel', extensions: ['xlsx', 'xltx', 'xls', 'xlsb', 'xlsm', 'xltm', 'xlt', 'xla', 'xlam'] }] }));
    mocks.save.mockResolvedValue(null); expect(await selectFile('desktop', 'export', true, 'copie.xlsx')).toBeNull();
    await expect(selectFile('demo', 'export', true)).rejects.toMatchObject({ code: 'DEMO_ONLY' });
  });
  it('subscribes and exposes cleanup for native drag/drop', async () => {
    const cleanup = vi.fn(); const onDrop = vi.fn(); const onHover = vi.fn();
    mocks.listen.mockImplementation(async (fn: (event: { payload: { type: string; paths?: string[] } }) => void) => { fn({ payload: { type: 'over' } }); fn({ payload: { type: 'drop', paths: ['C:\\Synthetic\\source.xlsx'] } }); return cleanup; });
    const stop = await subscribeDrop(onDrop, onHover); expect(onHover).toHaveBeenCalledWith(true); expect(onDrop).toHaveBeenCalledWith(['C:\\Synthetic\\source.xlsx']); stop(); expect(cleanup).toHaveBeenCalledOnce();
  });
  it('does not close the native window until save/discard is approved', async () => {
    let handler!: (event: { preventDefault: () => void }) => Promise<void>;
    const cleanup = vi.fn(); const canClose = vi.fn().mockResolvedValue(false); const reportError = vi.fn();
    mocks.closeListen.mockImplementation(async fn => { handler = fn; return cleanup; });
    const stop = await subscribeClose(canClose, reportError);
    const first = { preventDefault: vi.fn() }; await handler(first);
    expect(first.preventDefault).toHaveBeenCalledOnce(); expect(mocks.close).not.toHaveBeenCalled();
    canClose.mockResolvedValue(true);
    mocks.close.mockResolvedValue(undefined);
    await handler({ preventDefault: vi.fn() });
    expect(mocks.close).toHaveBeenCalledOnce(); expect(reportError).not.toHaveBeenCalled();
    stop(); expect(cleanup).toHaveBeenCalledOnce();
  });
});
