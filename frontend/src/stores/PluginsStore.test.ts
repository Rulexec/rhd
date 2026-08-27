import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PluginsStore } from './PluginsStore.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { PluginSummary } from '../lib/api/schemas.js';
import type { PluginListEventHandlers } from '../lib/api/chatApiImpl.js';

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

  beforeEach(() => {
    mockChatApi = {
      subscribePluginsList: vi.fn().mockResolvedValue(undefined),
      getPlugins: vi.fn().mockResolvedValue({ plugins: [mockPlugin] }),
      onPluginListEvents: vi.fn().mockReturnValue(() => {}),
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
});