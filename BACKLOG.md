# Backlog

## pending-acks-for-new-plugin

- Do not return pending acks for newly registered plugins. If a plugin was not registered when an event was emitted, the plugin should not receive that event via pending acks methods.
