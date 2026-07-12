import { describe, it, expect, beforeEach } from 'vitest';
import {
  availableRoles,
  activeRole,
  hasRoles,
  resetAllStores,
} from '../lib/chatStores';
import { get } from 'svelte/store';

describe('Role Stores', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('initializes with empty roles', () => {
    expect(get(availableRoles)).toEqual([]);
    expect(get(activeRole)).toBeNull();
    expect(get(hasRoles)).toBe(false);
  });

  it('updates available roles', () => {
    const roles = [
      {
        projectName: 'project-a',
        roleName: 'developer',
        whenToUse: 'Use for coding',
      },
    ];
    availableRoles.set(roles);
    expect(get(availableRoles)).toEqual(roles);
    expect(get(hasRoles)).toBe(true);
  });

  it('updates active role', () => {
    const role = { projectName: 'project-a', roleName: 'developer' };
    activeRole.set(role);
    expect(get(activeRole)).toEqual(role);
  });

  it('resetAllStores clears role state', () => {
    availableRoles.set([
      {
        projectName: 'project-a',
        roleName: 'developer',
        whenToUse: 'Use for coding',
      },
    ]);
    activeRole.set({ projectName: 'project-a', roleName: 'developer' });

    resetAllStores();

    expect(get(availableRoles)).toEqual([]);
    expect(get(activeRole)).toBeNull();
    expect(get(hasRoles)).toBe(false);
  });
});
