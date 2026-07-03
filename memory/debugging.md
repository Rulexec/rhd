# Debugging Guide

## Debugging Mode

When tests fail after 2 iterations of fixing, enter debugging mode.

### Rules

1. Stop trying blind fixes
2. Add debug logs around the issue area
3. All debug prints MUST have prefix `DBG:`
4. Run tests, analyze output
5. When issue found and test fixed — search all `DBG:` in project and remove them

### Debug Log Format

- Rust: `eprintln!("DBG: <context> = {:?}", value);`
- TypeScript: `console.log("DBG: <context>", value);`

### Cleanup

After fix: `rg "DBG:"` to find and remove all debug prints.
