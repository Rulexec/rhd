<script lang="ts">
  import { availableRoles, activeRole, hasRoles, currentChatId, isStreaming } from '@/lib/chatStores';
  import { dispatch } from '@/lib/actions';

  $: roles = $availableRoles;
  $: currentRole = $activeRole;
  $: chatId = $currentChatId;

  function handleRoleChange(event: Event) {
    const select = event.target as HTMLSelectElement;
    const value = select.value;
    
    if (value === '') {
      if (chatId) {
        dispatch({ type: 'clearActiveRole', payload: { chatId } });
      }
    } else {
      const [projectName, roleName] = value.split(':');
      if (chatId) {
        dispatch({
          type: 'setRole',
          payload: { chatId, projectName, roleName },
        });
      }
    }
  }

  $: selectedValue = currentRole
    ? `${currentRole.projectName}:${currentRole.roleName}`
    : '';
</script>

{#if $hasRoles && chatId}
  <div class="role-selector">
    <label for="role-select">Role:</label>
    <select
      id="role-select"
      value={selectedValue}
      on:change={handleRoleChange}
      disabled={$isStreaming}
    >
      <option value="">No role</option>
      {#each roles as role}
        <option value="{role.projectName}:{role.roleName}">
          {role.roleName} ({role.projectName})
        </option>
      {/each}
    </select>
  </div>
{/if}

<style>
  .role-selector {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem;
    border-bottom: 1px solid var(--border-color, #e0e0e0);
  }

  .role-selector label {
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--text-secondary, #666);
  }

  .role-selector select {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 4px;
    background: var(--bg-primary, #fff);
    color: var(--text-primary, #333);
    font-size: 0.875rem;
    cursor: pointer;
  }

  .role-selector select:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
