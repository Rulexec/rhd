# Custom Event Rejection and Context Fields - Implementation Plan

## Overview

This implementation plan adds two key features to the custom event system:

1. **Event Rejection**: Plugins can now reject custom events with `is_rejected: true`, allowing the event initiator to detect when plugins refuse to process an event.
2. **Context Fields**: Custom events can now include optional `chat_id`, `message_id`, and `tool_call_id` fields to provide better context about where the event originated.

## Implementation Phases

The implementation is divided into 4 sequential phases, each building on the previous one:

### Phase 1: Database Schema Changes
**File**: [`phase-1-database-schema.md`](./phase-1-database-schema.md)

Adds three new nullable columns to the `custom_events` table:
- `chat_id TEXT`
- `message_id TEXT`
- `tool_call_id TEXT`

Updates database layer functions to handle these new fields.

**Dependencies**: None (foundational phase)

### Phase 2: API Types Enhancement
**File**: [`phase-2-api-types.md`](./phase-2-api-types.md)

Updates API type definitions:
- `SendCustomEventParams`: Add optional context fields
- `CustomEventData`: Add optional context fields
- `AckCustomEventParams`: Add optional `is_rejected` field
- `CustomEventAcknowledgedData`: Add `is_rejected` field (defaults to false)
- `PendingEvent`: Add optional context fields

**Dependencies**: Phase 1

### Phase 3: Server Logic Implementation
**File**: [`phase-3-server-logic.md`](./phase-3-server-logic.md)

Updates server-side logic:
- `create_custom_event()`: Accept and store context fields
- `ack_custom_event()`: Handle `is_rejected` flag
- `get_pending_events()`: Include context fields in results
- Request handlers: Extract and pass new fields

**Dependencies**: Phase 1, Phase 2

### Phase 4: Integration Tests
**File**: [`phase-4-integration-tests.md`](./phase-4-integration-tests.md)

Comprehensive integration tests covering:
- Rejection flow (multiple plugins, mixed accept/reject)
- Context fields preservation through lifecycle
- Recovery via `getPendingAcks`
- Combined scenarios (context fields + rejection)
- Backward compatibility

**Dependencies**: Phase 3

## Key Design Decisions

### Rejection Mechanism
- **Approach**: Add `is_rejected` boolean field to existing acknowledgment event
- **Rationale**: Simpler than creating a separate rejection event type
- **Behavior**: Rejection by one plugin does NOT prevent other plugins from acknowledging

### Context Fields
- **Approach**: Add optional fields to all relevant structures
- **Rationale**: Provides better traceability without breaking existing code
- **Storage**: Fields are stored in database and included in recovery

### Backward Compatibility
- All new fields are optional
- Missing fields default to `None` (context fields) or `false` (is_rejected)
- Existing clients continue to work without changes

## Execution Order

```
Phase 1 → Phase 2 → Phase 3 → Phase 4
```

All phases must be executed sequentially. No parallelization is possible due to dependencies.

## Success Criteria

The implementation is complete when:

1. ✅ Plugins can send custom events with optional `chat_id`, `message_id`, and `tool_call_id` context fields
2. ✅ Context fields are broadcast to all subscribers via `customEvent`
3. ✅ Context fields are stored in the database and recovered via `getPendingAcks`
4. ✅ Plugins can acknowledge custom events with an optional `is_rejected` flag
5. ✅ Rejection by one plugin does not prevent other plugins from acknowledging the same event
6. ✅ The event initiator receives `customEventAcknowledged` events with the correct `is_rejected` value
7. ✅ All existing functionality remains unchanged (backward compatible)
8. ✅ All tests pass (unit tests, integration tests, database tests)
9. ✅ No breaking changes to the API contract

## Testing Strategy

- **Unit Tests**: Each phase includes unit tests for modified functions
- **Integration Tests**: Phase 4 adds comprehensive end-to-end tests
- **Database Tests**: Verify context field storage and retrieval
- **Backward Compatibility Tests**: Ensure old clients work without changes

## Files Modified

### Database Layer (`rhd_db`)
- `packages/rhd_db/src/chat_db/schema.rs`
- `packages/rhd_db/src/chat_db/custom_events.rs`
- `packages/rhd_db/src/chat_db/mod.rs`
- `packages/rhd_db/src/chat_db/tests/tags_plugins_events_tests.rs`

### API Types (`rhd_chat_api`)
- `packages/rhd_chat_api/src/methods/send_custom_event.rs`
- `packages/rhd_chat_api/src/events/custom_event.rs`
- `packages/rhd_chat_api/src/methods/ack_custom_event.rs`
- `packages/rhd_chat_api/src/events/custom_event_acknowledged.rs`
- `packages/rhd_chat_api/src/common.rs`

### Server Logic (`rhd_chat_server`)
- `packages/rhd_chat_server/src/custom_events.rs`
- `packages/rhd_chat_server/src/handlers/plugin.rs`
- `packages/rhd_chat_server/tests/websocket_tests.rs`

## Implementation Notes

- **No validation**: Context fields are informational only; the server doesn't validate that referenced entities exist
- **Pass-through logic**: The server stores and forwards context fields without transformation
- **Single acknowledgment**: Existing logic prevents duplicate acknowledgments; this remains unchanged
- **Default values**: Missing `is_rejected` defaults to `false` (acceptance)

## Related Documentation

- [Grand Plan](../custom-event-rejection-and-context-fields-grand-plan.md)
- [Plugins Feature Documentation](../../memory/features/plugins.md)
