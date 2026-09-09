import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PluginsStore } from './PluginsStore.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { PluginSummary, PluginState } from '../lib/api/schemas.js';
import type { PluginListEventHandlers, PluginStateEventHandlers } from '../lib/api/chatApiImpl.js';

/**
 * Unit tests for PluginsStore.
 *
 * Async methods are generator prototype methods that makeAutoObservable
 * auto-wraps into flows, so calling e.g. `store.loadPlugins()` returns a
 * CancellablePromise directly and can be awaited (see mobx-probe.test.ts
 * for the underlying MobX semantics).
 *
 * Plugin list events carry no chatVersion: onPluginRegistered/onPluginUpdated
 * receive `{ plugin }`, onPluginRemoved receives `{ pluginId }` (see
 * PluginListEventHandlers in chatApiImpl.ts).
 */
describe('PluginsStore', () => {
  let store: PluginsStore;
  let mockChatApi: ChatApi;

  const mockPlugin: PluginSummary = {
    pluginId: 'test-plugin',
    isActive: true
  };

  const mcpState: PluginState = {
    pluginId: 'mcp',
    key: 'status',
    content: '{"mcp":[]}',
    format: 'json',
    schema: 'mcpStatus:1',
    version: 1,
    updatedAt: '2026-09-05 22:41:07'
  };

  beforeEach(() => {
    mockChatApi = {
      subscribePluginsList: vi.fn().mockResolvedValue(undefined),
      getPlugins: vi.fn().mockResolvedValue({ plugins: [mockPlugin] }),
      onPluginListEvents: vi.fn().mockReturnValue(() => {}),
      getPluginStates: vi.fn().mockResolvedValue({ states: [] }),
      subscribePluginStates: vi.fn().mockResolvedValue({ states: [] }),
      unsubscribePluginStates: vi.fn().mockResolvedValue(undefined),
      onPluginStateEvents: vi.fn().mockReturnValue(() => {}),
      // Other ChatApi members are not exercised by these tests.
      subscribeChatsList: vi.fn(),
      unsubscribeChatsList: vi.fn(),
      listChats: vi.fn(),
      createChat: vi.fn(),
      deleteChat: vi.fn(),
      generateChatTitle: vi.fn(),
      onChatListEvents: vi.fn(),
      subscribeChat: vi.fn(),
      unsubscribeChat: vi.fn(),
      getChat: vi.fn(),
      onChatEvents: vi.fn(),
      getQueueMessages: vi.fn(),
      addQueueMessage: vi.fn(),
      onQueueMessageEvents: vi.fn()
    } as unknown as ChatApi;

    store = new PluginsStore({ chatApi: mockChatApi });
  });

  it('should initialize with empty state', () => {
    expect(store.plugins).toEqual([]);
    expect(store.hasPlugins).toBe(false);
    expect(store.activePluginsCount).toBe(0);
    expect(store.loading).toBe(false);
    expect(store.error).toBe(null);
  });

  it('should load plugins', async () => {
    await store.loadPlugins();

    expect(mockChatApi.getPlugins).toHaveBeenCalled();
    expect(store.plugins).toEqual([mockPlugin]);
    expect(store.hasPlugins).toBe(true);
    expect(store.loading).toBe(false);
  });

  it('should handle load error', async () => {
    vi.mocked(mockChatApi.getPlugins).mockRejectedValueOnce(new Error('Load failed'));

    await store.loadPlugins();

    expect(store.error).toBe('Load failed');
    expect(store.loading).toBe(false);
  });

  it('should sort plugins alphabetically by pluginId', async () => {
    const plugin1: PluginSummary = { pluginId: 'zebra-plugin', isActive: true };
    const plugin2: PluginSummary = { pluginId: 'alpha-plugin', isActive: true };

    vi.mocked(mockChatApi.getPlugins).mockResolvedValueOnce({ plugins: [plugin1, plugin2] });
    await store.loadPlugins();

    expect(store.plugins[0]!.pluginId).toBe('alpha-plugin');
    expect(store.plugins[1]!.pluginId).toBe('zebra-plugin');
  });

  it('should count active plugins', async () => {
    const activePlugin: PluginSummary = { pluginId: 'active', isActive: true };
    const inactivePlugin: PluginSummary = { pluginId: 'inactive', isActive: false };

    vi.mocked(mockChatApi.getPlugins).mockResolvedValueOnce({
      plugins: [activePlugin, inactivePlugin]
    });
    await store.loadPlugins();

    expect(store.activePluginsCount).toBe(1);
  });

  it('should handle plugin registered event', async () => {
    let onPluginRegisteredHandler: PluginListEventHandlers['onPluginRegistered'];
    vi.mocked(mockChatApi.onPluginListEvents).mockImplementation((handlers) => {
      onPluginRegisteredHandler = handlers.onPluginRegistered;
      return () => {};
    });

    await store.init();

    const newPlugin: PluginSummary = { pluginId: 'new-plugin', isActive: true };
    onPluginRegisteredHandler!({ plugin: newPlugin });

    expect(store.plugins.some(p => p.pluginId === 'new-plugin')).toBe(true);
  });

  it('should update an existing plugin on registered event', async () => {
    let onPluginRegisteredHandler: PluginListEventHandlers['onPluginRegistered'];
    vi.mocked(mockChatApi.onPluginListEvents).mockImplementation((handlers) => {
      onPluginRegisteredHandler = handlers.onPluginRegistered;
      return () => {};
    });

    await store.init();

    const duplicate: PluginSummary = { pluginId: 'test-plugin', isActive: false };
    onPluginRegisteredHandler!({ plugin: duplicate });

    expect(store.plugins).toHaveLength(1);
    expect(store.plugins[0]!.isActive).toBe(false);
  });

  it('should handle plugin updated event', async () => {
    let onPluginUpdatedHandler: PluginListEventHandlers['onPluginUpdated'];
    vi.mocked(mockChatApi.onPluginListEvents).mockImplementation((handlers) => {
      onPluginUpdatedHandler = handlers.onPluginUpdated;
      return () => {};
    });

    await store.init();

    const updatedPlugin: PluginSummary = { pluginId: 'test-plugin', isActive: false };
    onPluginUpdatedHandler!({ plugin: updatedPlugin });

    expect(store.plugins[0]!.isActive).toBe(false);
  });

  it('should handle plugin removed event', async () => {
    let onPluginRemovedHandler: PluginListEventHandlers['onPluginRemoved'];
    vi.mocked(mockChatApi.onPluginListEvents).mockImplementation((handlers) => {
      onPluginRemovedHandler = handlers.onPluginRemoved;
      return () => {};
    });

    await store.init();
    expect(store.plugins.length).toBe(1);

    onPluginRemovedHandler!({ pluginId: 'test-plugin' });

    expect(store.plugins.length).toBe(0);
  });

  it('should cleanup on clear', async () => {
    const cleanup = vi.fn();
    vi.mocked(mockChatApi.onPluginListEvents).mockReturnValue(cleanup);

    await store.init();
    store.clear();

    expect(cleanup).toHaveBeenCalled();
    expect(store.plugins).toEqual([]);
  });

  describe('plugin states', () => {
    /** Capture the state event handlers registered during init(). */
    function captureStateHandlers(): () => PluginStateEventHandlers {
      let handlers: PluginStateEventHandlers = {};
      vi.mocked(mockChatApi.onPluginStateEvents).mockImplementation((h) => {
        handlers = h;
        return () => {};
      });
      return () => handlers;
    }

    it('should load states on init', async () => {
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({ states: [mcpState] });

      await store.init();

      expect(mockChatApi.getPluginStates).toHaveBeenCalled();
      expect(store.statesFor('mcp')).toEqual([mcpState]);
      expect(store.hasMcpStatus).toBe(true);
    });

    it('should subscribe with held versions', async () => {
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({ states: [mcpState] });

      await store.init();

      expect(mockChatApi.subscribePluginStates).toHaveBeenCalledWith([
        { pluginId: 'mcp', key: 'status', version: 1 }
      ]);
    });

    it('should apply catch-up response from subscribe', async () => {
      const v2State: PluginState = { ...mcpState, version: 2 };
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({ states: [mcpState] });
      vi.mocked(mockChatApi.subscribePluginStates).mockResolvedValueOnce({ states: [v2State] });

      await store.init();

      expect(store.statesFor('mcp')).toEqual([v2State]);
    });

    it('should apply live change event', async () => {
      const getHandlers = captureStateHandlers();
      await store.init();

      const v2State: PluginState = { ...mcpState, version: 2 };
      getHandlers().onPluginStateChanged!({ state: v2State });

      expect(store.statesFor('mcp')).toEqual([v2State]);
    });

    it('should ignore stale events', async () => {
      const v2State: PluginState = { ...mcpState, version: 2 };
      const getHandlers = captureStateHandlers();
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({ states: [v2State] });
      await store.init();

      getHandlers().onPluginStateChanged!({ state: mcpState }); // v1 — stale

      expect(store.statesFor('mcp')).toEqual([v2State]);
    });

    it('should apply removal event', async () => {
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({ states: [mcpState] });
      const getHandlers = captureStateHandlers();
      await store.init();

      getHandlers().onPluginStateRemoved!({ pluginId: 'mcp', key: 'status', version: 3 });

      expect(store.statesFor('mcp')).toEqual([]);
      expect(store.hasMcpStatus).toBe(false);
    });

    it('should ignore stale removal', async () => {
      const v5State: PluginState = { ...mcpState, version: 5 };
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({ states: [v5State] });
      const getHandlers = captureStateHandlers();
      await store.init();

      getHandlers().onPluginStateRemoved!({ pluginId: 'mcp', key: 'status', version: 4 });

      expect(store.statesFor('mcp')).toEqual([v5State]);
    });

    it('should drop states when plugin removed', async () => {
      let onPluginRemovedHandler: PluginListEventHandlers['onPluginRemoved'];
      vi.mocked(mockChatApi.onPluginListEvents).mockImplementation((handlers) => {
        onPluginRemovedHandler = handlers.onPluginRemoved;
        return () => {};
      });
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({ states: [mcpState] });
      await store.init();
      expect(store.statesFor('mcp')).toHaveLength(1);

      onPluginRemovedHandler!({ pluginId: 'mcp' });

      expect(store.statesFor('mcp')).toEqual([]);
    });

    it('should reset states and unsubscribe state events on clear', async () => {
      const stateCleanup = vi.fn();
      vi.mocked(mockChatApi.onPluginStateEvents).mockReturnValue(stateCleanup);
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({ states: [mcpState] });
      await store.init();

      store.clear();

      expect(stateCleanup).toHaveBeenCalled();
      expect(store._states).toEqual([]);
      expect(store.hasMcpStatus).toBe(false);
    });

    it('should set store.error when loading states fails', async () => {
      vi.mocked(mockChatApi.getPluginStates).mockRejectedValueOnce(new Error('States load failed'));

      await store.init();

      expect(store.error).toBe('States load failed');
    });

    it('should filter states by schema via statesWithSchema', async () => {
      const errorState: PluginState = {
        ...mcpState,
        key: 'errors',
        schema: 'errors:1',
        version: 1
      };
      vi.mocked(mockChatApi.getPluginStates).mockResolvedValueOnce({
        states: [mcpState, errorState]
      });

      await store.init();

      expect(store.statesWithSchema('mcpStatus:1')).toEqual([mcpState]);
      expect(store.statesWithSchema('errors:1')).toEqual([errorState]);
      expect(store.statesFor('mcp').map(s => s.key)).toEqual(['errors', 'status']);
    });
  });
});