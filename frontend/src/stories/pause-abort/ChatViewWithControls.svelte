<script lang="ts">
  import ChatView from '@/components/ChatView.svelte';
  import StoryExecutionControls from '@/stories/StoryExecutionControls.svelte';
  import type { StoryControlDefinition } from '@/stories/StoryExecutionControls.svelte';
  
  let { story }: { story: StoryControlDefinition<any> } = $props();
</script>

<div style="display: flex; flex-direction: column; height: 100vh;">
  <StoryExecutionControls {story} />
  <div style="flex: 1; overflow: hidden;">
    <ChatView />
  </div>
</div>
