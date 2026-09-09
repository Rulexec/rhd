<script lang="ts">
  import TabView, { type TabType } from './TabView.svelte';

  interface Props {
    activeTab?: TabType;
    showMcps?: boolean;
    onTabChange?: (detail: { tab: TabType }) => void;
  }

  let { activeTab = 'chats', showMcps = false, onTabChange }: Props = $props();

  function handleTabChange(event: CustomEvent<{ tab: TabType }>) {
    onTabChange?.(event.detail);
  }
</script>

<TabView {activeTab} {showMcps} on:tabChange={handleTabChange} />
