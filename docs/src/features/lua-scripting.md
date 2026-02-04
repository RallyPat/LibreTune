# Lua Scripting

LibreTune includes a sandboxed Lua runtime for lightweight automation. Use the Lua Console for quick calculations, custom utilities, or future action scripting.

## Lua Console

Open **Tools → Lua Console** to run scripts. The console shows output from `print()` and the script’s return value.

Example:

- `print('Hello from Lua')`
- `return 2 + 2`

The output area shows:

- `Hello from Lua`
- `=> 4`

## Sandbox Limits

The Lua runtime is sandboxed:

- **No file I/O**
- **No network access**
- **No OS-level commands**

This keeps scripts safe and predictable in tuning environments.

## Best Practices

- Use Lua for lightweight math or logic.
- Keep scripts short and targeted.
- Prefer table edits and built-in tools for tune changes.
