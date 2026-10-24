import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Command, Message } from './types';
export const native = () => '__TAURI_INTERNALS__' in window;
export async function request(request: Command): Promise<Message> {
  if (!native())
    throw new Error('Daemon indisponível nesta visualização web. Abra o aplicativo Padsmith.');
  const result = await invoke<Message>('daemon_request', { request });
  if (result.event === 'error') throw new Error(result.message);
  return result;
}
export async function subscribe(callback: (message: Message) => void, offline: () => void) {
  if (!native()) return () => {};
  const a = await listen<Message>('padsmith', (event) => callback(event.payload));
  const b = await listen('padsmith-offline', offline);
  return () => {
    a();
    b();
  };
}
export { invoke };
