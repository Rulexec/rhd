import { makeAutoObservable } from 'mobx';
import { ApiError } from '../lib/api/ProxyLogsApi.js';
import type { ProxyLogsApi } from '../lib/api/ProxyLogsApi.js';
import type { ChatDetail, ChatSummary, RequestDetail } from '../lib/api/schemas.js';
import { yieldPromise } from '../util/async.js';

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Root store of the proxy-logs viewer.
 *
 * Async operations are generator prototype methods (auto-wrapped into flows by
 * makeAutoObservable — calling store.loadChats() returns a CancellablePromise
 * awaitable directly; use flowResult(...) when an explicitly-typed promise is
 * wanted, e.g. from components). Reactive state is stored in public fields.
 */
export class ProxyLogsStore {
  #api: ProxyLogsApi;

  // ---- chats list area -------------------------------------------------
  /** Raw chat list as served (already updated_at DESC). Public for MobX. */
  _chats: ChatSummary[] = [];
  chatsLoading: boolean = false;
  chatsError: string | null = null;

  // ---- chat detail area ------------------------------------------------
  selectedChatId: number | null = null;
  chatDetail: ChatDetail | null = null;
  detailLoading: boolean = false;
  detailError: string | null = null;

  // ---- request detail area ---------------------------------------------
  selectedRequestId: number | null = null;
  requestDetail: RequestDetail | null = null;

  // ---- refresh ----------------------------------------------------------
  refreshing: boolean = false;

  constructor(options: { api: ProxyLogsApi }) {
    this.#api = options.api;
    makeAutoObservable(this);
  }

  get chats(): ChatSummary[] {
    return this._chats;
  }

  get hasChats(): boolean {
    return this._chats.length > 0;
  }

  /** Summary of the selected chat, or null when it is no longer in the list. */
  get selectedChat(): ChatSummary | null {
    return this._chats.find((chat) => chat.id === this.selectedChatId) ?? null;
  }

  /** Load the chat list (initial load; the refresh button uses refresh()). */
  *loadChats(): Generator<unknown, void, unknown> {
    this.chatsLoading = true;
    this.chatsError = null;
    try {
      const result = yield* yieldPromise(this.#api.listChats());
      this._chats = result.chats;
    } catch (error) {
      this.chatsError = errorMessage(error);
    } finally {
      this.chatsLoading = false;
    }
  }

  /** Open a chat: set selection, drop any open request, fetch its detail. */
  *openChat(chatId: number): Generator<unknown, void, unknown> {
    this.selectedChatId = chatId;
    this.selectedRequestId = null;
    this.requestDetail = null;
    this.chatDetail = null;
    this.detailError = null;
    this.detailLoading = true;
    try {
      const detail = yield* yieldPromise(this.#api.getChatDetail(chatId));
      this.chatDetail = detail;
    } catch (error) {
      this.detailError = errorMessage(error);
    } finally {
      this.detailLoading = false;
    }
  }

  /** Open one request's raw detail (timeline row click). */
  *openRequest(requestId: number): Generator<unknown, void, unknown> {
    this.selectedRequestId = requestId;
    this.requestDetail = null;
    try {
      const detail = yield* yieldPromise(this.#api.getRequestDetail(requestId));
      this.requestDetail = detail;
    } catch (error) {
      // Surface in the detail area; the timeline stays usable.
      this.detailError = errorMessage(error);
    }
  }

  /** Close the request drill-down (does not touch the chat selection). */
  closeRequest(): void {
    this.selectedRequestId = null;
    this.requestDetail = null;
  }

  /**
   * Refresh button: re-fetch everything currently visible, preserving the
   * selection. A selected chat that no longer exists after the refresh clears
   * the selection (same for a selected request missing from the refreshed
   * chat detail).
   */
  *refresh(): Generator<unknown, void, unknown> {
    this.refreshing = true;
    this.chatsError = null;
    try {
      const result = yield* yieldPromise(this.#api.listChats());
      this._chats = result.chats;

      if (this.selectedChatId !== null) {
        const stillExists = this._chats.some((chat) => chat.id === this.selectedChatId);
        if (!stillExists) {
          this.clearSelection();
        } else {
          const detail = yield* yieldPromise(this.#api.getChatDetail(this.selectedChatId));
          this.chatDetail = detail;
          this.detailError = null;
        }
      }

      if (this.selectedRequestId !== null) {
        const stillPresent =
          this.chatDetail?.requests.some((r) => r.id === this.selectedRequestId) ?? false;
        if (!stillPresent) {
          this.closeRequest();
        } else {
          this.requestDetail = yield* yieldPromise(
            this.#api.getRequestDetail(this.selectedRequestId)
          );
        }
      }
    } catch (error) {
      // A 404 mid-refresh means the selection vanished server-side; treat as
      // a selection change, not an error banner.
      if (error instanceof ApiError && error.status === 404) {
        this.clearSelection();
      } else {
        this.chatsError = errorMessage(error);
      }
    } finally {
      this.refreshing = false;
    }
  }

  /** Drop chat + request selection and related detail state. */
  clearSelection(): void {
    this.selectedChatId = null;
    this.chatDetail = null;
    this.detailError = null;
    this.closeRequest();
  }
}
