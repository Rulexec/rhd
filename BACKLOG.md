## daemon-completion-logs

I want `rhd daemon` to emit logs every time we execute a request to AI completions. It should be brief, just with context (like whether it's called from a scenario or from chat, include IDs if there are any).

## system-prompt-tags

I see following line of code:

```
m.role == "system" && m.content.contains("rhd_set_todo_list Tool Contract")
```

I do not like it, since system prompt may be changed in the future and we will resend new contract.

So let's add a `tags` field to system prompts. When you are adding the prompt for todos — add the tag `rhdTodoContract`. Then you can check if this exists.

For project and roles prompts, do the same thing. For project it can be, for example, `project:<projectName>:systemPrompt`. For roles you can add multiple tags: `rolesList`, `rolesList:<roleName>:entry`. So you can check if we have already added a system prompt for new roles — check that the chat contains system messages with the `rolesList:<roleName>:entry` tag.

Make them visible on the frontend, so the user can better see why this system message is added to the chat.

## websocket-retries

I started frontend without `rhd daemon`. It spams with websocket connections, but not closing previous connection, so I see a bunch of pending connections. Let terminate previous attempt when starting new. Also, increase time between retries to 5 seconds at least.

## pause-abort-scenarios

We need to fix pause-abort scenarios. Currently their implementation is almost completely broken. Both pause and abort should do one final thing: stop tool loop execution. They have only one difference: pause just waits for last tool/AI chat to finish (so tool loop will be paused just after last AI chat request tool results are available), abort terminates all current tool calls or current AI chat request and pauses chat. After resume it depends on state when we were:

- If there were tool calls which are aborted — we need to emit error tool results to AI chat with tool result's message "Aborted"
- If it was just paused without abort — resume should behave like we never paused this chat
- If AI chat request was aborted — we should just forget it, as it never existed

User can add messages while chat is paused. Then after resume — they should be sent. When we will implement it, ask user to provide examples.

## storybook-addon-vitest

Replace `@storybook/test-runner` with `@storybook/addon-vitest` for running Storybook interaction tests.
