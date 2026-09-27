<script lang="ts">
  import ConversationView from './ConversationView.svelte';
  import type { ConversationTurn } from '../api/schemas.js';

  interface Props {
    conversation: ConversationTurn[];
    onRequestSelect?: (detail: { requestId: number }) => void;
  }

  let { conversation, onRequestSelect }: Props = $props();
</script>

<ConversationView
  {conversation}
  on:requestSelect={(e) => onRequestSelect?.(e.detail)}
/>
