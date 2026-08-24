<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import commonStyles from '../styles/common.module.css';

  interface Props {
    title?: string;
    message?: string;
    confirmText?: string;
    cancelText?: string;
    confirmVariant?: 'primary' | 'danger';
    onConfirm: () => void;
    onCancel: () => void;
  }

  let {
    title = 'Confirm',
    message = 'Are you sure?',
    confirmText = 'Confirm',
    cancelText = 'Cancel',
    confirmVariant = 'primary',
    onConfirm,
    onCancel
  }: Props = $props();

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onCancel();
    }
  }

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      onCancel();
    }
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    document.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class={commonStyles['modal-overlay']} onclick={handleOverlayClick} role="dialog" aria-modal="true" aria-labelledby="modal-title">
  <div class={commonStyles['modal']}>
    <div class={commonStyles['modal-header']}>
      <h2 id="modal-title">{title}</h2>
    </div>
    <div class={commonStyles['modal-body']}>
      <p>{message}</p>
    </div>
    <div class={commonStyles['modal-footer']}>
      <button class={commonStyles['btn']} onclick={onCancel}>
        {cancelText}
      </button>
      <button
        class="{commonStyles['btn']} {confirmVariant === 'danger' ? commonStyles['btn-danger'] : commonStyles['btn-primary']}"
        onclick={onConfirm}
      >
        {confirmText}
      </button>
    </div>
  </div>
</div>
