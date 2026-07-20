export function parseAssistantMessage(msg: any): any {
  if (msg.role !== 'assistant') return msg;
  
  try {
    const json = JSON.parse(msg.content);
    if (json.content !== undefined && json.toolCalls) {
      const toolCalls = json.toolCalls.length > 0 ? json.toolCalls.map((tc: any) => ({
        id: tc.id,
        name: tc.function?.name || tc.name,
        arguments: tc.function?.arguments || tc.arguments,
        status: tc.status || 'pending',
        mcpId: tc.mcpId || (tc.function?.name || tc.name || '').split('/')[0],
        result: tc.result,
      })) : undefined;
      
      return {
        ...msg,
        content: json.content,
        toolCalls,
      };
    }
  } catch {
    // Not JSON, use as-is
  }
  
  return msg;
}

export function parseToolResult(content: string): { toolCallId: string; result: string; isError: boolean } | null {
  try {
    const toolResult = JSON.parse(content);
    if (toolResult.toolCallId) {
      return {
        toolCallId: toolResult.toolCallId,
        result: toolResult.result,
        isError: toolResult.isError || false,
      };
    }
  } catch {
    // Not JSON
  }
  return null;
}

export function mergeToolResults(messages: any[]): any[] {
  const toolResults = new Map<string, { result: string; isError: boolean }>();
  
  for (const msg of messages) {
    if (msg.role === 'tool') {
      const parsed = parseToolResult(msg.content);
      if (parsed) {
        toolResults.set(parsed.toolCallId, { result: parsed.result, isError: parsed.isError });
      }
    }
  }
  
  const result: any[] = [];
  for (const msg of messages) {
    if (msg.role === 'tool') {
      const parsed = parseToolResult(msg.content);
      if (parsed) {
        continue;
      }
    }
    
    let processed = parseAssistantMessage(msg);
    
    if (processed.toolCalls && processed.toolCalls.length > 0) {
      const updatedToolCalls = processed.toolCalls.map((tc: any) => {
        const toolResult = toolResults.get(tc.id);
        if (toolResult) {
          return {
            ...tc,
            result: toolResult.result,
            status: toolResult.isError ? 'failed' as const : 'completed' as const,
          };
        }
        return tc;
      });
      processed = { ...processed, toolCalls: updatedToolCalls };
    }
    
    result.push(processed);
  }
  
  return result;
}
