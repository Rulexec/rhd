# Backlog

## Chat Messages Queue

Add a `messages_queue` to chat alongside the existing `messages` collection. The queue should support similar CRUD operations as regular messages, with corresponding events emitted for queue modifications (add, update, delete).

## Chat Tools Management

Implement tool management for chat. Reference `rhd_ai` for tool definition format, but copy the types to `rhd_chat_api` rather than reusing them directly.
