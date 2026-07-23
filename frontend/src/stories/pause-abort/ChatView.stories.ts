import type { Meta, StoryObj } from '@storybook/svelte';
import ChatView from '@/components/ChatView.svelte';
import { chats, currentChatId, messages, availableModels, selectedModel, isStreaming, streamError, streamingMessageId, isPaused, pendingToolCalls, availableRoles, activeRole, todoList, resetAllStores } from '@/lib/chatStores';
import { chatProjects, mcpStatuses } from '@/lib/projectStores';

const mockChat = {
  id: 1,
  title: 'Test Chat',
  createdAt: '2026-07-23T10:00:00Z',
  updatedAt: '2026-07-23T10:00:00Z',
  activeModel: 'gpt-4',
};

const mockMessages = [
  {
    id: 1,
    chatId: 1,
    role: 'user' as const,
    content: 'Hello, how are you?',
    createdAt: '2026-07-23T10:00:00Z',
    model: 'gpt-4',
    thinkingContent: null,
  },
  {
    id: 2,
    chatId: 1,
    role: 'assistant' as const,
    content: 'I\'m doing well, thank you! How can I help you today?',
    createdAt: '2026-07-23T10:00:05Z',
    model: 'gpt-4',
    thinkingContent: null,
  },
];

function setupDefaultStores() {
  resetAllStores();
  chats.set([mockChat]);
  currentChatId.set(1);
  messages.set(mockMessages);
  availableModels.set(['gpt-4', 'gpt-3.5-turbo']);
  selectedModel.set('gpt-4');
  isStreaming.set(false);
  streamError.set(null);
  streamingMessageId.set(null);
  isPaused.set(false);
  pendingToolCalls.set([]);
  availableRoles.set([]);
  activeRole.set(null);
  todoList.set([]);
  chatProjects.set([]);
  mcpStatuses.set([]);
}

setupDefaultStores();

const meta = {
  title: 'Pause-Abort/ChatView',
  component: ChatView,
  tags: ['autodocs'],
  parameters: {
    layout: 'fullscreen',
  },
  decorators: [
    (Story) => {
      setupDefaultStores();
      return Story();
    },
  ],
} satisfies Meta<ChatView>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

