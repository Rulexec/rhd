export type TabId = 'scenarios' | 'chats';

export interface ParsedHash {
  tab: TabId;
  chatId: number | null;
}

export function parseHash(): ParsedHash {
  const hash = window.location.hash.slice(1);

  if (!hash) {
    return { tab: 'scenarios', chatId: null };
  }

  const parts = hash.split('/');
  const tab = parts[0] as TabId;

  if (tab !== 'scenarios' && tab !== 'chats') {
    return { tab: 'scenarios', chatId: null };
  }

  const chatId = parts[1] ? parseInt(parts[1], 10) : null;
  const validChatId = chatId !== null && !isNaN(chatId) ? chatId : null;

  return { tab, chatId: validChatId };
}

export function updateHash(tab: TabId, chatId?: number | null): void {
  let newHash = `#${tab}`;
  if (chatId) {
    newHash += `/${chatId}`;
  }

  if (window.location.hash !== newHash) {
    window.location.hash = newHash;
  }
}

export function initRouter(onHashChange: (parsed: ParsedHash) => void): () => void {
  const handleHashChange = () => {
    const parsed = parseHash();
    onHashChange(parsed);
  };

  window.addEventListener('hashchange', handleHashChange);

  return () => {
    window.removeEventListener('hashchange', handleHashChange);
  };
}
