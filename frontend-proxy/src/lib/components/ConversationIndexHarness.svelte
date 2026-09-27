<script lang="ts">
  import ConversationIndex from './ConversationIndex.svelte';
  import type { ConversationTurn } from '../api/schemas.js';

  interface Props {
    conversation: ConversationTurn[];
    onEntrySelect?: (detail: { seq: number }) => void;
  }

  let { conversation, onEntrySelect }: Props = $props();
</script>

<ConversationIndex
  {conversation}
  on:entrySelect={(e) => onEntrySelect?.(e.detail)}
/>
