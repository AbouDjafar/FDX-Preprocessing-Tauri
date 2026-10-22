import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import type { Op, Operations, Progress, Request } from '../model';
import { DemoAdapter, DEMO_DICTIONARY, DEMO_SOURCE } from './demo';

export interface Adapter {
  readonly mode: 'desktop' | 'demo';
  request<K extends Op>(request: Request<K>, onProgress: (progress: Progress) => void): Promise<Operations[K]['result']>;
  cancel(taskId: string): Promise<void>;
}
export class DesktopAdapter implements Adapter {
  readonly mode = 'desktop' as const;
  async request<K extends Op>(request: Request<K>, onProgress: (progress: Progress) => void): Promise<Operations[K]['result']> {
    const onEvent = new Channel<Progress>();
    onEvent.onmessage = event => { if (event.task_id === request.task_id) onProgress(event); };
    try { return await invoke<Operations[K]['result']>('fdx', { request, onEvent }); }
    finally { onEvent.onmessage = () => {}; }
  }
  cancel(taskId: string): Promise<void> { return invoke('cancel_task', { taskId }); }
}
// A failing desktop bridge must never silently switch a real project to fixtures.
export function createAdapter(): Adapter { return isTauri() ? new DesktopAdapter() : new DemoAdapter(); }
export type FileKind = 'source' | 'dictionary' | 'profile' | 'project' | 'export';
const filters: Record<FileKind, { name: string; extensions: string[] }[]> = {
  source: [{ name: 'Classeur Excel', extensions: ['xlsx'] }], dictionary: [{ name: 'Dictionnaire métier', extensions: ['md', 'txt'] }],
  profile: [{ name: 'Profil FDX', extensions: ['json'] }], project: [{ name: 'Projet FDX', extensions: ['fdxproj'] }], export: [{ name: 'Classeur Excel', extensions: ['xlsx'] }],
};
export async function selectFile(mode: Adapter['mode'], kind: FileKind, save = false, name?: string): Promise<string | null> {
  if (mode === 'demo') {
    if (kind === 'source') return DEMO_SOURCE;
    if (kind === 'dictionary') return DEMO_DICTIONARY;
    if (kind === 'export') throw { code: 'DEMO_ONLY', message: 'La démonstration ne produit aucun fichier Excel.' };
    return `/DEMO/${kind === 'profile' ? 'profil-demo.json' : (name || 'projet-demo.fdxproj')}`;
  }
  const dialog = await import('@tauri-apps/plugin-dialog');
  if (save) return dialog.save({ title: 'Enregistrer une copie', defaultPath: name, filters: filters[kind] });
  const selected = await dialog.open({ title: 'Ouvrir un fichier', directory: false, multiple: false, filters: filters[kind] });
  return typeof selected === 'string' ? selected : null;
}
export async function subscribeDrop(onDrop: (paths: string[]) => void, onHover: (hover: boolean) => void): Promise<() => void> {
  const { getCurrentWebview } = await import('@tauri-apps/api/webview');
  return getCurrentWebview().onDragDropEvent(({ payload }) => {
    onHover(payload.type === 'over' || payload.type === 'enter');
    if (payload.type === 'drop') onDrop(payload.paths);
  });
}
