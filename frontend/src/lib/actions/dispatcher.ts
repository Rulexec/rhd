import type { ChatAction } from './types';
import { processAction } from './processors';

type ActionOverride = (action: ChatAction) => void | Promise<void>;

const overrides = new Map<string, ActionOverride>();

export function _testOverrideAction(actionType: string, handler: ActionOverride): void {
  overrides.set(actionType, handler);
}

export function _testClearOverrides(): void {
  overrides.clear();
}

export async function dispatch(action: ChatAction): Promise<void> {
  const override = overrides.get(action.type);
  if (override) {
    await override(action);
    return;
  }
  await processAction(action);
}
