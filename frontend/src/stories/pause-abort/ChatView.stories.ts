import type { Meta, StoryObj } from '@storybook/svelte';
import ChatView from '@/components/ChatView.svelte';
import ChatViewWithControls from './ChatViewWithControls.svelte';
import type { StoryControlDefinition } from '@/stories/StoryExecutionControls.svelte';
import { 
  chats, 
  currentChatId, 
  messages, 
  availableModels, 
  selectedModel, 
  isStreaming, 
  streamError, 
  streamingMessageId, 
  isPaused, 
  pendingToolCalls, 
  availableRoles, 
  activeRole, 
  todoList, 
  resetAllStores 
} from '@/lib/chatStores';
import { chatProjects, mcpStatuses } from '@/lib/projectStores';
import { setMockWsHandler, clearMockWsHandlers, emitWsEvent } from '@/stories/mockWs';
import { TEST_IDS } from '@/stories/testIds';
import { sleep } from '@/stories/testUtils';

const mockChat = {
  id: 1,
  title: 'Test Chat',
  createdAt: '2026-07-23T10:00:00Z',
  updatedAt: '2026-07-23T10:00:00Z',
  activeModel: 'gpt-4',
};

interface StoryState {
  messageId: number;
}

function setupDefaultStores() {
  resetAllStores();
  chats.set([mockChat]);
  currentChatId.set(1);
  messages.set([]);
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

const storyDefinition: StoryControlDefinition<StoryState> = {
  getInitialState: () => ({
    messageId: 1,
  }),
  
  steps: [
    {
      name: 'Add user message',
      execute: async ({ state }) => {
        setMockWsHandler('sendMessage', async (request) => {
          return { type: 'response', id: request.id as string, success: true, data: {} };
        });

        const textarea = document.querySelector(`[data-testid="${TEST_IDS.MESSAGE_INPUT}"]`) as HTMLTextAreaElement;
        const sendButton = document.querySelector(`[data-testid="${TEST_IDS.SEND_BUTTON}"]`) as HTMLButtonElement;
        
        if (!textarea || !sendButton) {
          throw new Error('Could not find message input or send button');
        }
        
        textarea.value = 'Hello, how are you?';
        textarea.dispatchEvent(new Event('input', { bubbles: true }));
        
        await sleep(100);
        
        sendButton.click();
        
        await sleep(200);
        
        return { state: { ...state, messageId: state.messageId + 1 } };
      },
    },
    
    {
      name: 'Receive AI response',
      execute: async ({ state }) => {
        await sleep(100);
        emitWsEvent('chatStreamChunk', {
          chatId: 1,
          content: 'I\'m doing well, thank you! ',
        });
        
        await sleep(100);
        emitWsEvent('chatStreamChunk', {
          chatId: 1,
          content: 'How can I help you today?',
        });
        
        await sleep(100);
        emitWsEvent('chatStreamFinished', {
          chatId: 1,
        });
        
        await sleep(200);
        
        clearMockWsHandlers();
        
        return { state: { ...state, messageId: state.messageId + 1 } };
      },
    },
  ],
};

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

export const WithControls: Story = {
  render: () => ({
    Component: ChatViewWithControls,
    props: {
      story: storyDefinition,
    },
  }),
};
