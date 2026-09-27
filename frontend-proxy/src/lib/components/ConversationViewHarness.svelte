<script lang="ts">
  import ConversationView from './ConversationView.svelte';
  import type { ConversationTurn, RequestDetail } from '../api/schemas.js';

  interface Props {
    conversation: ConversationTurn[];
    requestDetail?: RequestDetail | null;
    openAnchor?: number | 'tail' | null;
    onRequestSelect?: (detail: { requestId: number; anchor: number | 'tail' }) => void;
    onCloseRequest?: () => void;
  }

  let { conversation, requestDetail, openAnchor, onRequestSelect, onCloseRequest }: Props =
    $props();
</script>

<ConversationView
  {conversation}
  {requestDetail}
  {openAnchor}
  {onCloseRequest}
  on:requestSelect={(e) => onRequestSelect?.(e.detail)}
/>
