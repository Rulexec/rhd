export { parseAssistantMessage, parseToolResult, mergeToolResults } from './parsers';
export {
  loadChats,
  loadAvailableModels,
  createChat,
  selectChat,
  deleteChat,
  deleteAllChats,
  sendMessage,
  queueMessage,
  editMessage,
  abortChat,
  loadAvailableRoles,
  setRole,
  clearActiveRole,
  pauseChat,
  resumeChat,
} from './operations';
export { handleChatEvent } from './events';

// Test-only exports for state-based testing
export {
  loadChats as _test_loadChats,
  loadAvailableModels as _test_loadAvailableModels,
  createChat as _test_createChat,
  selectChat as _test_selectChat,
  deleteChat as _test_deleteChat,
  deleteAllChats as _test_deleteAllChats,
  sendMessage as _test_sendMessage,
  queueMessage as _test_queueMessage,
  editMessage as _test_editMessage,
  abortChat as _test_abortChat,
  pauseChat as _test_pauseChat,
  resumeChat as _test_resumeChat,
} from './operations';
export {
  handleChatEvent as _test_handleChatEvent,
} from './events';
