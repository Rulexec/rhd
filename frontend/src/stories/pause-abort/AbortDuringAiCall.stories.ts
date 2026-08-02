import type { Meta, StoryObj } from '@storybook/svelte';
import { get } from 'svelte/store';
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
  isAborted,
  pendingToolCalls, 
  pendingMessages,
  availableRoles, 
  activeRole, 
  todoList, 
  resetAllStores 
} from '@/lib/chatStores';
import { chatProjects, mcpStatuses } from '@/lib/projectStores';
import { setMockWsHandler, clearMockWsHandlers, emitWsEvent } from '@/stories/mockWs';
import { TEST_IDS } from '@/stories/testIds';
import { waitFor } from '@/stories/testUtils';
import { dispatch } from '@/lib/actions';

const mockChat = {
  id: 1,
  title: 'Test Chat',
  createdAt: '2026-07-23T10:00:00Z',
  updatedAt: '2026-07-23T10:00:00Z',
  activeModel: 'gpt-4',
};

interface StoryState {
  step: number;
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
  isAborted.set(false);
  pendingToolCalls.set([]);
  pendingMessages.set([]);
  availableRoles.set([]);
  activeRole.set(null);
  todoList.set([]);
  chatProjects.set([]);
  mcpStatuses.set([]);
}

const storyDefinition: StoryControlDefinition<StoryState> = {
  getInitialState: () => ({
    step: 0,
  }),
  
  steps: [
    {
      name: 'setup',
      execute: async ({ state }) => {
        setupDefaultStores();
        
        // Add a user message to simulate an ongoing conversation
        const userMessage = {
          id: 1,
          chatId: 1,
          role: 'user' as const,
          content: 'Hello, can you help me?',
          createdAt: new Date().toISOString(),
          model: 'gpt-4',
        };
        messages.set([userMessage]);
        
        // Set streaming state to simulate AI responding
        isStreaming.set(true);
        isPaused.set(false);
        isAborted.set(false);
        streamingMessageId.set(null); // Show streaming indicator
        
        setMockWsHandler('abortChat', async (request) => {
          return { type: 'response', id: request.id as string, success: true, data: {} };
        });
        
        setMockWsHandler('resumeChat', async (request) => {
          return { type: 'response', id: request.id as string, success: true, data: {} };
        });
        
        setMockWsHandler('queueMessage', async (request) => {
          return { type: 'response', id: request.id as string, success: true, data: {} };
        });
        
        await waitFor(() => {
          if (get(isStreaming) !== true) throw new Error('isStreaming should be true');
          if (get(isPaused) !== false) throw new Error('isPaused should be false');
          if (get(isAborted) !== false) throw new Error('isAborted should be false');
          if (get(messages).length !== 1) throw new Error('Should have 1 message');
        });
        
        return { state: { ...state, step: 1 } };
      },
    },
    
    {
      name: 'click abort',
      execute: async ({ state }) => {
        const abortButton = document.querySelector(`[data-testid="${TEST_IDS.ABORT_BUTTON}"]`) as HTMLButtonElement;
        
        if (!abortButton) {
          throw new Error('Could not find abort button');
        }
        
        abortButton.click();
        
        return { state: { ...state, step: 2 } };
      },
    },
    
    {
      name: 'daemon response: streamAborted',
      execute: async ({ state }) => {
        dispatch({ type: 'streamAborted' });
        
        await waitFor(() => {
          if (get(isPaused) !== true) throw new Error('isPaused should be true');
          if (get(isAborted) !== true) throw new Error('isAborted should be true');
          if (get(isStreaming) !== false) throw new Error('isStreaming should be false');
          if (get(streamingMessageId) !== null) throw new Error('streamingMessageId should be null');
        });
        
        return { state: { ...state, step: 3 } };
      },
    },
    
    {
      name: 'type message',
      execute: async ({ state }) => {
        const textarea = document.querySelector(`[data-testid="${TEST_IDS.MESSAGE_INPUT}"]`) as HTMLTextAreaElement;
        
        if (!textarea) {
          throw new Error('Could not find message input');
        }
        
        textarea.value = 'Please continue later';
        textarea.dispatchEvent(new Event('input', { bubbles: true }));
        
        return { state: { ...state, step: 4 } };
      },
    },
    
    {
      name: 'click send (queue)',
      execute: async ({ state }) => {
        const sendButton = document.querySelector(`[data-testid="${TEST_IDS.SEND_BUTTON}"]`) as HTMLButtonElement;
        
        if (!sendButton) {
          throw new Error('Could not find send button');
        }
        
        sendButton.click();
        
        await waitFor(() => {
          if (get(pendingMessages).length !== 1) throw new Error('pendingMessages should have length 1');
        });
        
        return { state: { ...state, step: 5 } };
      },
    },
    
    {
      name: 'click resume',
      execute: async ({ state }) => {
        const resumeButton = document.querySelector(`[data-testid="${TEST_IDS.RESUME_BUTTON}"]`) as HTMLButtonElement;
        
        if (!resumeButton) {
          throw new Error('Could not find resume button');
        }
        
        resumeButton.click();
        
        return { state: { ...state, step: 6 } };
      },
    },
    
    {
      name: 'daemon response: chatResumed',
      execute: async ({ state }) => {
        // The aborted AI call was cancelled, so on resume the daemon immediately
        // starts a new call with the queued message. Promotion happens here.
        dispatch({ type: 'chatResumed' });
        
        await waitFor(() => {
          if (get(isPaused) !== false) throw new Error('isPaused should be false');
          if (get(isAborted) !== false) throw new Error('isAborted should be false');
          if (get(isStreaming) !== true) throw new Error('isStreaming should be true');
          if (get(pendingMessages).length !== 0) throw new Error('pendingMessages should be empty (promoted on chatResumed)');
          const promoted = get(messages).find((m) => m.role === 'user' && m.content === 'Please continue later');
          if (!promoted) throw new Error('messages should contain the promoted user message');
        });
        
        return { state: { ...state, step: 7 } };
      },
    },
    
    {
      name: 'verify final state',
      execute: async ({ state }) => {
        await waitFor(() => {
          if (get(isPaused) !== false) throw new Error('isPaused should be false');
          if (get(isStreaming) !== true) throw new Error('isStreaming should be true (new stream started for queued message)');
        });
        
        clearMockWsHandlers();
        
        return { state: { ...state, step: 8 } };
      },
    },
  ],
};

setupDefaultStores();

const meta = {
  title: 'Pause-Abort/AbortDuringAiCall',
  component: ChatView,
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

export const WithControls: Story = {
  render: () => ({
    Component: ChatViewWithControls,
    props: {
      story: storyDefinition,
    },
  }),
};
