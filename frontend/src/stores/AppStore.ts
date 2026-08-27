import { makeAutoObservable } from 'mobx';
import { ConnectionStore } from './ConnectionStore.js';
import { ChatsListStore } from './ChatsListStore.js';
import { ChatStore } from './ChatStore.js';
import { PluginsStore } from './PluginsStore.js';
import type { ChatApi } from '../lib/api/ChatApi.js';

/**
 * Root application store.
 * Provides access to all substores via lazy getters.
 *
 * NOTE: Substore lazy getters (chatsList, chat, plugins) are added in
 * Phases 3-5 as their store classes are implemented.
 */
export class AppStore {
  #chatApi: ChatApi;
  #connectionStore?: ConnectionStore;
  #chatsListStore?: ChatsListStore;
  #chatStore?: ChatStore;
  #pluginsStore?: PluginsStore;

  constructor(options: { chatApi: ChatApi }) {
    this.#chatApi = options.chatApi;
    makeAutoObservable(this);
  }

  /**
   * Connection store for WebSocket connection state.
   * ConnectionStore does not depend on the chat API, so it is constructed standalone.
   */
  get connection(): ConnectionStore {
    if (!this.#connectionStore) {
      this.#connectionStore = new ConnectionStore();
    }
    return this.#connectionStore;
  }

  /**
   * Chat list store (lazy initialized).
   */
  get chatsList(): ChatsListStore {
    if (!this.#chatsListStore) {
      this.#chatsListStore = new ChatsListStore({ chatApi: this.#chatApi });
    }
    return this.#chatsListStore;
  }

  /**
   * Current chat store (lazy initialized).
   */
  get chat(): ChatStore {
    if (!this.#chatStore) {
      this.#chatStore = new ChatStore({ chatApi: this.#chatApi });
    }
    return this.#chatStore;
  }

  /**
   * Plugins store (lazy initialized).
   */
  get plugins(): PluginsStore {
    if (!this.#pluginsStore) {
      this.#pluginsStore = new PluginsStore({ chatApi: this.#chatApi });
    }
    return this.#pluginsStore;
  }
}