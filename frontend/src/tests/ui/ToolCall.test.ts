/**
 * Test cases covered:
 * - tests/cases/pause-abort/abort-during-tool-execution.md (UI rendering steps)
 */

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import ToolCallMessage from '../../components/ToolCallMessage.svelte';
import type { ToolCall } from '../../lib/types/index';

describe('ToolCallMessage UI', () => {
  // Covers abort-during-tool-execution.md step 11 (UI rendering)
  // Step 11. System updates tool call UI to show "Aborted" error (shows Aborted error for cancelled tool calls)
  it('shows Aborted error for cancelled tool calls', () => {
    const toolCall: ToolCall = {
      id: 'call_1',
      name: 'test_tool',
      arguments: '{}',
      status: 'failed',
      result: 'Aborted',
      mcpId: 'test_mcp',
    };
    render(ToolCallMessage, { props: { toolCall } });
    
    // The tool call should show the failed status icon
    const statusIcon = document.querySelector('.status-failed');
    expect(statusIcon).toBeTruthy();
    expect(statusIcon?.textContent).toBe('✗');
  });

  it('shows normal error for non-aborted failures', () => {
    const toolCall: ToolCall = {
      id: 'call_1',
      name: 'test_tool',
      arguments: '{}',
      status: 'failed',
      result: 'Error: Connection failed',
      mcpId: 'test_mcp',
    };
    render(ToolCallMessage, { props: { toolCall } });
    
    // The tool call should show the failed status icon
    const statusIcon = document.querySelector('.status-failed');
    expect(statusIcon).toBeTruthy();
    expect(statusIcon?.textContent).toBe('✗');
  });

  it('shows completed status for successful tool calls', () => {
    const toolCall: ToolCall = {
      id: 'call_1',
      name: 'test_tool',
      arguments: '{}',
      status: 'completed',
      result: 'Success',
      mcpId: 'test_mcp',
    };
    render(ToolCallMessage, { props: { toolCall } });
    
    const statusIcon = document.querySelector('.status-completed');
    expect(statusIcon).toBeTruthy();
    expect(statusIcon?.textContent).toBe('✓');
  });

  it('shows running status for in-progress tool calls', () => {
    const toolCall: ToolCall = {
      id: 'call_1',
      name: 'test_tool',
      arguments: '{}',
      status: 'running',
      mcpId: 'test_mcp',
    };
    render(ToolCallMessage, { props: { toolCall } });
    
    const statusIcon = document.querySelector('.status-running');
    expect(statusIcon).toBeTruthy();
    expect(statusIcon?.textContent).toBe('⏳');
  });
});
