import * as proxyLogsApi from './proxyLogsApiImpl.js';

/**
 * Proxy logs API interface for dependency injection. Allows mocking in tests.
 */
export interface ProxyLogsApi {
  listChats: typeof proxyLogsApi.listChats;
  getChatDetail: typeof proxyLogsApi.getChatDetail;
  getRequestDetail: typeof proxyLogsApi.getRequestDetail;
}

/** Default implementation backed by the Vite dev-server middleware (Phase 2). */
export const defaultProxyLogsApi: ProxyLogsApi = {
  listChats: proxyLogsApi.listChats,
  getChatDetail: proxyLogsApi.getChatDetail,
  getRequestDetail: proxyLogsApi.getRequestDetail
};

export { ApiError } from './proxyLogsApiImpl.js';
