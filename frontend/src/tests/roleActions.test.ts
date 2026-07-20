import { describe, it, expect, beforeEach, vi } from 'vitest';
import { _testOverrideAction, _testClearOverrides } from '../lib/actions';
import { availableRoles, activeRole, resetAllStores } from '../lib/chatStores';
import { get } from 'svelte/store';
import type { ChatAction } from '../lib/actions/types';

describe('Role Actions', () => {
  beforeEach(() => {
    _testClearOverrides();
    resetAllStores();
  });

  it('handles roleChanged action', async () => {
    _testOverrideAction('roleChanged', async (action) => {
      const payload = (action as Extract<ChatAction, { type: 'roleChanged' }>).payload;
      activeRole.set({
        projectName: payload.projectName,
        roleName: payload.roleName,
      });
    });

    const { dispatch } = await import('../lib/actions');
    await dispatch({
      type: 'roleChanged',
      payload: {
        chatId: 1,
        projectName: 'project-a',
        roleName: 'developer',
      },
    });

    expect(get(activeRole)).toEqual({
      projectName: 'project-a',
      roleName: 'developer',
    });
  });

  it('handles rolesUpdated action', async () => {
    _testOverrideAction('rolesUpdated', async (action) => {
      const payload = (action as Extract<ChatAction, { type: 'rolesUpdated' }>).payload;
      availableRoles.set(payload.roles);
      if (payload.activeRoleProject && payload.activeRoleName) {
        activeRole.set({
          projectName: payload.activeRoleProject,
          roleName: payload.activeRoleName,
        });
      }
    });

    const { dispatch } = await import('../lib/actions');
    await dispatch({
      type: 'rolesUpdated',
      payload: {
        chatId: 1,
        roles: [
          {
            projectName: 'project-a',
            roleName: 'developer',
            whenToUse: 'Use for coding',
          },
        ],
        activeRoleProject: 'project-a',
        activeRoleName: 'developer',
      },
    });

    expect(get(availableRoles)).toHaveLength(1);
    expect(get(activeRole)).toEqual({
      projectName: 'project-a',
      roleName: 'developer',
    });
  });
});
