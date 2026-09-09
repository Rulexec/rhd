import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import { tick } from 'svelte';
import { runInAction } from 'mobx';
import McpStatusListHarness from './McpStatusListHarness.svelte';
import { PluginsStore } from '../../stores/PluginsStore.js';
import type { AppStore } from '../../stores/AppStore.js';
import type { ChatApi } from '../api/ChatApi.js';
import type { PluginStateEventHandlers } from '../api/chatApiImpl.js';
import type { PluginState, PluginSummary } from '../api/schemas.js';

const mcpPlugin: PluginSummary = { pluginId: 'mcp', isActive: true };
const mcp2Plugin: PluginSummary = { pluginId: 'mcp2', isActive: true };
const inactiveMcpPlugin: PluginSummary = { pluginId: 'mcp', isActive: false };

function payloadJson(entries: unknown[]): string {
  return JSON.stringify({ mcp: entries });
}

function mcpState(overrides: Partial<PluginState> = {}): PluginState {
  return {
    pluginId: 'mcp',
    key: 'status',
    content: payloadJson([{ id: 'fs', name: 'filesystem', status: 'ok' }]),
    format: 'json',
    schema: 'mcpStatus:1',
    version: 1,
    updatedAt: '2026-09-05 22:41:07',
    ...overrides
  };
}

/**
 * Build a real PluginsStore backed by a mocked ChatApi (same approach as the
 * PluginList tests). init() populates plugins and states through the real
 * get → subscribe flow; the state event handlers registered during init are
 * captured so tests can push live changes into the store.
 */
function createFixture(options: { plugins?: PluginSummary[]; states?: PluginState[] }) {
  let stateHandlers: PluginStateEventHandlers = {};
  const mockChatApi = {
    subscribePluginsList: vi.fn().mockResolvedValue(undefined),
    getPlugins: vi.fn().mockResolvedValue({ plugins: options.plugins ?? [] }),
    onPluginListEvents: vi.fn().mockReturnValue(() => {}),
    getPluginStates: vi.fn().mockResolvedValue({ states: options.states ?? [] }),
    subscribePluginStates: vi.fn().mockResolvedValue({ states: [] }),
    unsubscribePluginStates: vi.fn().mockResolvedValue(undefined),
    onPluginStateEvents: vi.fn().mockImplementation((handlers: PluginStateEventHandlers) => {
      stateHandlers = handlers;
      return () => {};
    })
  } as unknown as ChatApi;

  const plugins = new PluginsStore({ chatApi: mockChatApi });
  const appStore = { plugins } as unknown as AppStore;

  return {
    plugins,
    appStore,
    emitStateChanged: (state: PluginState) =>
      runInAction(() => stateHandlers.onPluginStateChanged?.({ state }))
  };
}

afterEach(() => {
  cleanup();
});

describe('McpStatusList', () => {
  it('renders server rows from parsed payload', async () => {
    const { appStore } = createFixture({
      plugins: [mcpPlugin],
      states: [
        mcpState({
          content: payloadJson([
            { id: 'fs', name: 'filesystem', status: 'ok' },
            { id: 'bad', name: 'broken', status: 'error', error: 'spawn failed' }
          ])
        })
      ]
    });
    await appStore.plugins.init();

    const { container, getByText, queryByText } = render(McpStatusListHarness, {
      props: { appStore }
    });

    const rows = container.querySelectorAll('.mcp-entry');
    expect(rows.length).toBe(2);
    expect(getByText('filesystem')).toBeTruthy();
    expect(getByText('broken')).toBeTruthy();
    expect(getByText('fs')).toBeTruthy();
    expect(getByText('OK')).toBeTruthy();
    expect(getByText('Error')).toBeTruthy();
    expect(getByText('spawn failed')).toBeTruthy();
    // The ok row carries no error message element
    expect(rows[0]!.querySelector('.mcp-entry-error-msg')).toBeNull();
    expect(queryByText('No plugin is publishing MCP status')).toBeNull();
  });

  it('groups states by plugin', async () => {
    const { appStore } = createFixture({
      plugins: [mcpPlugin, mcp2Plugin],
      states: [
        mcpState({ content: payloadJson([{ id: 'a', name: 'alpha', status: 'ok' }]) }),
        mcpState({
          pluginId: 'mcp2',
          content: payloadJson([{ id: 'b', name: 'beta', status: 'ok' }])
        })
      ]
    });
    await appStore.plugins.init();

    const { container, getByText } = render(McpStatusListHarness, { props: { appStore } });

    const sections = container.querySelectorAll('section.mcp-group');
    expect(sections.length).toBe(2);
    const titles = [...container.querySelectorAll('.mcp-group-title')].map(t =>
      t!.textContent!.replace(/\s+/g, ' ').trim()
    );
    expect(titles).toContain('mcp v1');
    expect(titles).toContain('mcp2 v1');
    expect(getByText('alpha')).toBeTruthy();
    expect(getByText('beta')).toBeTruthy();
  });

  it('falls back to raw view for unparseable content', async () => {
    const { appStore } = createFixture({
      plugins: [mcpPlugin],
      states: [mcpState({ content: 'garbage' })]
    });
    await appStore.plugins.init();

    const { container, getByText } = render(McpStatusListHarness, { props: { appStore } });

    const parseError = container.querySelector('.mcp-parse-error');
    expect(parseError).toBeTruthy();
    expect(parseError!.textContent).toContain('Unrecognized mcpStatus:1 payload');
    expect(getByText('garbage')).toBeTruthy();
  });

  it('reacts to a live state update', async () => {
    const { appStore, emitStateChanged } = createFixture({
      plugins: [mcpPlugin],
      states: [
        mcpState({
          version: 1,
          content: payloadJson([
            { id: 'bad', name: 'broken', status: 'error', error: 'spawn failed' }
          ])
        })
      ]
    });
    await appStore.plugins.init();

    const { getByText, queryByText } = render(McpStatusListHarness, {
      props: { appStore }
    });
    expect(getByText('Error')).toBeTruthy();
    expect(getByText('spawn failed')).toBeTruthy();

    emitStateChanged(
      mcpState({
        version: 2,
        content: payloadJson([{ id: 'bad', name: 'broken', status: 'ok' }])
      })
    );
    await tick();

    expect(getByText('OK')).toBeTruthy();
    expect(queryByText('Error')).toBeNull();
    expect(queryByText('spawn failed')).toBeNull();
    expect(getByText('v2')).toBeTruthy();
  });

  it('marks states of an inactive plugin as last known', async () => {
    const { appStore } = createFixture({
      plugins: [inactiveMcpPlugin],
      states: [mcpState()]
    });
    await appStore.plugins.init();

    const { container } = render(McpStatusListHarness, { props: { appStore } });

    const stale = container.querySelector('.mcp-group-stale');
    expect(stale).toBeTruthy();
    expect(stale!.textContent).toBe('last known');
  });
});
