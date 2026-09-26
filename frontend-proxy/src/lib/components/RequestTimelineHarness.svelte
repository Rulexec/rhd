<script lang="ts">
  import RequestTimeline from './RequestTimeline.svelte';
  import type { RequestSummary } from '../api/schemas.js';

  interface Props {
    requests: RequestSummary[];
    selectedRequestId?: number | null;
    onRequestSelect?: (detail: { requestId: number }) => void;
  }

  let { requests, selectedRequestId = null, onRequestSelect }: Props = $props();
</script>

<RequestTimeline
  {requests}
  {selectedRequestId}
  on:requestSelect={(e) => onRequestSelect?.(e.detail)}
/>
