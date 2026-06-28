<script>
  export let content;
</script>

<div class="message assistant streaming">
  {#if content}
    <div class="content">{content}</div>
  {:else}
    <div class="loading">
      <span class="dot"></span>
      <span class="dot"></span>
      <span class="dot"></span>
    </div>
  {/if}
</div>

<style>
  .message {
    margin-bottom: var(--spacing-m);
    padding: var(--spacing-m);
    border-radius: 8px;
    margin-right: 40px;
    background: var(--color-bg);
    border: 1px solid var(--color-border);
  }

  .content {
    white-space: pre-wrap;
    word-wrap: break-word;
    line-height: 1.6;
  }

  .loading {
    display: flex;
    gap: 4px;
    padding: var(--spacing-s) 0;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-text-muted);
    animation: bounce 1.4s infinite ease-in-out both;
  }

  .dot:nth-child(1) {
    animation-delay: -0.32s;
  }

  .dot:nth-child(2) {
    animation-delay: -0.16s;
  }

  @keyframes bounce {
    0%, 80%, 100% {
      transform: scale(0);
    }
    40% {
      transform: scale(1);
    }
  }
</style>
