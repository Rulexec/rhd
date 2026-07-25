<script lang="ts">
  import ChatView from '@/components/ChatView.svelte';
  import StoryExecutionControls from '@/stories/StoryExecutionControls.svelte';
  import type { StoryControlDefinition } from '@/stories/StoryExecutionControls.svelte';
  
  let { story }: { story: StoryControlDefinition<any> } = $props();
</script>

<div class="story-container">
  <StoryExecutionControls {story} />
  <div class="chat-view-wrapper">
    <ChatView />
  </div>
</div>

<style>
  .story-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  
  .chat-view-wrapper {
    flex: 1;
    overflow: hidden;
  }
</style>
