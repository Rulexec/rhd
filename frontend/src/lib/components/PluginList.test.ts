import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import { tick } from 'svelte';
import { runInAction } from 'mobx';
import PluginListHarness from './PluginListHarness.svelte';
import { PluginsStore } from '../../stores/PluginsStore.js';
import type { AppStore } from '../../stores/AppStore.js';
import type { ChatApi } from '../api/ChatApi.js';
import type { PluginStateEventHandlers } from '../api/chatApiImpl.js';
import type { PluginState, PluginSummary } from '../api/schemas.js';

const activePlugin: PluginSummary = { pluginId: 'test-plugin', isActive: true };
const inactivePlugin: PluginSummary = { pluginId: 'gone-plugin', isActive: false };

function markdownState(overrides: Partial<PluginState> = {}): PluginState {
  return {
    pluginId: 'test-plugin',
    key: 'readme',
    content: '# Hello',
    format: 'markdown',
    schema: '',
    version: 2,
    updatedAt: '2026-09-05 22:41:07',
    ...overrides
  };
}

function jsonState(overrides: Partial<PluginState> = {}): PluginState {
  return {
    pluginId: 'test-plugin',
    key: 'status',
    content: '{"a":1}',
    format: 'json',
    schema: 'mcpStatus:1',
    version: 3,
    updatedAt: '2026-09-05 22:41:07',
    ...overrides
  };
}

/**
 * Build a real PluginsStore backed by a mocked ChatApi (preferred over a plain
 * fake substore for fidelity — see phase-8 sub-plan). init() populates plugins
 * and states through the same get → subscribe flow the app uses; the state
 * event handlers registered during init are captured so tests can push live
 * changes into the store.
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
    // runInAction mirrors what the app should do around event-handler-driven
    // mutations: the store's private apply helpers are not MobX-annotated
    // (#-methods are invisible to makeAutoObservable), and the mounted
    // component observes _states, so a bare call would trip strict mode.
    emitStateChanged: (state: PluginState) =>
      runInAction(() => stateHandlers.onPluginStateChanged?.({ state }))
  };
}

afterEach(() => {
  cleanup();
});

describe('PluginList states section', () => {
  it('renders no state section for plugin without states', async () => {
    const { appStore } = createFixture({ plugins: [activePlugin], states: [] });
    await appStore.plugins.init();

    const { container, getByText } = render(PluginListHarness, { props: { appStore } });

    expect(getByText('test-plugin')).toBeTruthy();
    expect(container.querySelector('details')).toBeNull();
  });

  it('renders the state section collapsed by default with a count', async () => {
    const { appStore } = createFixture({
      plugins: [activePlugin],
      states: [markdownState()]
    });
    await appStore.plugins.init();

    const { container, getByText } = render(PluginListHarness, { props: { appStore } });

    const details = container.querySelector('details');
    expect(details).toBeTruthy();
    expect(details!.hasAttribute('open')).toBe(false);
    expect(getByText('State (1)')).toBeTruthy();
    expect(getByText('readme')).toBeTruthy();
  });

  it('renders markdown content via marked after expand', async () => {
    const { appStore } = createFixture({
      plugins: [activePlugin],
      states: [markdownState({ content: '# Hello' })]
    });
    await appStore.plugins.init();

    const { container } = render(PluginListHarness, { props: { appStore } });

    const details = container.querySelector('details')!;
    details.open = true;

    const heading = container.querySelector('.markdown h1');
    expect(heading).toBeTruthy();
    expect(heading!.textContent).toBe('Hello');
  });

  it('pretty-prints json content', async () => {
    const { appStore } = createFixture({
      plugins: [activePlugin],
      states: [jsonState({ content: '{"a":1}', schema: '' })]
    });
    await appStore.plugins.init();

    const { container } = render(PluginListHarness, { props: { appStore } });

    const pre = container.querySelector('pre');
    expect(pre).toBeTruthy();
    expect(pre!.textContent).toBe('{\n  "a": 1\n}');
  });

  it('falls back to raw content for malformed json', async () => {
    const { appStore } = createFixture({
      plugins: [activePlugin],
      states: [jsonState({ content: 'not-json', schema: '' })]
    });
    await appStore.plugins.init();

    const { container } = render(PluginListHarness, { props: { appStore } });

    const pre = container.querySelector('pre');
    expect(pre!.textContent).toBe('not-json');
  });

  it('shows format, schema and version badges', async () => {
    const { appStore } = createFixture({
      plugins: [activePlugin],
      states: [jsonState()]
    });
    await appStore.plugins.init();

    const { container, getByText } = render(PluginListHarness, { props: { appStore } });

    expect(getByText('json')).toBeTruthy();
    expect(getByText('mcpStatus:1')).toBeTruthy();
    expect(getByText('v3')).toBeTruthy();
    expect(container.querySelector('.plugin-state-schema')).toBeTruthy();
  });

  it('marks states of an inactive plugin as last known', async () => {
    const { appStore } = createFixture({
      plugins: [inactivePlugin],
      states: [markdownState({ pluginId: 'gone-plugin' })]
    });
    await appStore.plugins.init();

    const { container } = render(PluginListHarness, { props: { appStore } });

    const stale = container.querySelector('.plugin-state-stale');
    expect(stale).toBeTruthy();
    expect(stale!.textContent).toBe('last known');
  });

  it('reacts to a live state change event without re-setup', async () => {
    const { appStore, emitStateChanged } = createFixture({
      plugins: [activePlugin],
      states: []
    });
    await appStore.plugins.init();

    const { container } = render(PluginListHarness, { props: { appStore } });
    expect(container.querySelector('details')).toBeNull();

    emitStateChanged(jsonState());
    await tick();

    expect(container.querySelector('details')).toBeTruthy();
    expect(container.textContent).toContain('State (1)');
    expect(container.textContent).toContain('v3');
  });
});
