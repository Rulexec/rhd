export const TEST_IDS = {
  MESSAGE_INPUT: 'message-input',
  SEND_BUTTON: 'send-button',
  PAUSE_BUTTON: 'pause-button',
  RESUME_BUTTON: 'resume-button',
} as const;

export const STEP_BUTTON_PREFIX = 'step-button';

export function stepButtonTestId(stepIndex: number): string {
  return `${STEP_BUTTON_PREFIX}-${stepIndex}`;
}
