import { writable, get } from 'svelte/store';
import type { Project, McpStatus, ChatProject } from './types/index';
import { sendRequest, generateRequestId } from './ws';
import { currentChatId } from './chatStores';

export const projects: import('svelte/store').Writable<Project[]> = writable([]);
export const mcpStatuses: import('svelte/store').Writable<McpStatus[]> = writable([]);
export const chatProjects: import('svelte/store').Writable<ChatProject[]> = writable([]);

export async function loadProjects(): Promise<void> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'listProjects', id });
  if (response.success) {
    projects.set(response.data || []);
  }
}

export async function loadMcpStatus(projectName: string): Promise<void> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getProjectMcpStatus', id, projectName });
  if (response.success) {
    const statuses = response.data || [];
    mcpStatuses.update((list) => {
      const filtered = list.filter((s) => s.projectName !== projectName);
      return [...filtered, ...statuses];
    });
  }
}

export async function loadChatProjects(chatId: number): Promise<void> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getChatProjects', id, chatId });
  if (response.success) {
    chatProjects.set(response.data || []);
  }
}

export async function attachProject(chatId: number, projectName: string): Promise<{ success: boolean; error?: string }> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'attachProject', id, chatId, projectName });
  if (response.success) {
    chatProjects.update((list) => [...list, { name: projectName, systemPromptAdded: false }]);
    return { success: true };
  }
  return { success: false, error: response.error || 'Failed to attach project' };
}

export async function detachProject(chatId: number, projectName: string): Promise<void> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'detachProject', id, chatId, projectName });
  if (response.success) {
    chatProjects.update((list) => list.filter((p) => p.name !== projectName));
    mcpStatuses.update((list) => list.filter((s) => s.projectName !== projectName));
  }
}

export function handleMcpStatusEvent(data: unknown): void {
  const event = data as { projectName: string; mcpId: string; status: 'connecting' | 'connected' | 'failed'; error?: string };
  mcpStatuses.update((list) => {
    const idx = list.findIndex((s) => s.projectName === event.projectName && s.mcpId === event.mcpId);
    if (idx !== -1) {
      const updated = [...list];
      updated[idx] = { ...updated[idx], status: event.status, error: event.error };
      return updated;
    }
    return [...list, event];
  });
}

export function handleProjectAttachedEvent(data: unknown): void {
  const event = data as { chatId: number; projectName: string };
  const currentId = get(currentChatId);
  if (currentId === event.chatId) {
    chatProjects.update((list) => {
      if (list.some((p) => p.name === event.projectName)) {
        return list;
      }
      return [...list, { name: event.projectName, systemPromptAdded: false }];
    });
  }
}

export function handleProjectDetachedEvent(data: unknown): void {
  const event = data as { chatId: number; projectName: string };
  const currentId = get(currentChatId);
  if (currentId === event.chatId) {
    chatProjects.update((list) => list.filter((p) => p.name !== event.projectName));
    mcpStatuses.update((list) => list.filter((s) => s.projectName !== event.projectName));
  }
}
