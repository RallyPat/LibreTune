# MCP Server

LibreTune can expose its tune-inspection tools to **external AI agents** over
a local [Model Context Protocol](https://modelcontextprotocol.io) server. An
agent running in Claude Code, Claude Desktop, or any other MCP client can then
read the tune you have open and reason about it, using the same deterministic
tools as the built-in [AI Assistant](./ai-assistant.md).

> **Read-only.** The MCP server exposes no tool that can edit a tune, stage a
> proposal, or touch the ECU. An outside agent can look; only you can change
> anything.

The server is **off by default** and only ever listens on `127.0.0.1` — it is
never reachable from another machine.

## Enabling the server

1. Open **Tools → Settings**.
2. Scroll to **AI Assistant (at your own risk) → MCP server (local)**.
3. Tick **Expose tools over MCP**.

The server starts immediately and the checkbox label shows the bound address,
e.g. `Running on 127.0.0.1:8765`. Untick it, or close LibreTune, to stop it.
The setting is remembered, so a server left enabled restarts on the next
launch.

Unlike the rest of the Settings dialog, the MCP controls apply the moment you
use them rather than on **OK** — each one binds or releases a real socket.

## Authentication

Every request must carry a bearer token:

```
Authorization: Bearer <token>
```

The token is unique per installation, 64 hex characters, and shown under
**Access token** in the settings section (click **Show**). It is stored in
`mcp-token` in LibreTune's app data directory, readable only by your user
account on macOS and Linux, and is never written to `settings.json` or to any
log.

Click **Regenerate** to mint a fresh token. The old one stops working
immediately: if the server is running, it restarts on the same port with the
new token. Update your client configuration afterwards.

## Available tools

All eight are read-only. They are the same tools the in-app assistant uses, so
what an external agent sees can never drift from what the assistant sees.

| Tool | What it returns |
|------|-----------------|
| `list_tables` | Every editable table with its role (VE / Ignition / AFR target / …) and dimensions. Call this first to discover names. |
| `read_table` | Current values, axis bins, and units for one table. |
| `read_constant` | One constant's value, min/max, units, and options. |
| `list_features` | Feature-toggle (bits) constants and their available options. |
| `summarize_tune_context` | Per-cell AFR error, data coverage, suspect cells, and unexplored cells for a VE table. |
| `tune_health_check` | Overall health score and per-region coverage for a table. |
| `get_realtime_snapshot` | Current sensor values (RPM, MAP, TPS, CLT, AFR, …). Requires a live connection. |
| `query_datalog` | Per-channel statistics, or the last rows, from the current session or a saved log. |

Values come back in **raw ECU units**, not your display preferences — an
external model has no display context, and a silently converted number would
be worse than useless to something doing arithmetic on it.

The assistant's `propose_*` tools are deliberately **not** exposed. A proposal
only means something inside LibreTune's review queue, and an agent calling one
of those names over MCP is refused rather than quietly ignored.

## Connecting Claude Code

```bash
claude mcp add --transport http libretune http://127.0.0.1:8765/mcp \
  --header "Authorization: Bearer TOKEN"
```

Replace `TOKEN` with the value from **Access token**. The settings section
also renders this command with your current port filled in — click it to
select, then copy.

## Connecting Claude Desktop

Claude Desktop's config UI cannot send static headers, so route it through the
`mcp-remote` bridge (requires Node.js):

```json
{
  "mcpServers": {
    "libretune": {
      "command": "npx",
      "args": ["mcp-remote", "http://127.0.0.1:8765/mcp", "--allow-http",
               "--transport", "http-only",
               "--header", "Authorization:${AUTH_HEADER}"],
      "env": { "AUTH_HEADER": "Bearer TOKEN" }
    }
  }
}
```

The space after `Authorization:` belongs inside the environment variable, as
written above — it works around an argument-escaping quirk in Claude Desktop.

## Changing the port

Set **Port** (minimum 1024) and click **Apply port**. The server restarts on
the new port if it was running. If the port is already taken, the error
appears under the controls and the server stays stopped — pick another one, or
free the port (`lsof -i :8765` on macOS/Linux, `netstat -ano` on Windows).

## Security

- **Loopback only.** The listener binds `127.0.0.1`, never `0.0.0.0`.
- **Constant-time token check.** The bearer token is compared without an early
  exit, so its bytes cannot be recovered from response timing.
- **Host and Origin validation.** Both are pinned to loopback forms, blocking
  DNS-rebinding attacks from a browser page.
- **No write surface.** There is no `apply`, no `burn`, and no `propose` tool
  to call.

A failed request gets a bare `401` with no body — it never echoes back what
was sent.

## Example session

1. Open a project in LibreTune and enable the MCP server.
2. Connect Claude Code with the command above.
3. Ask it something that needs the tune:
   - *"Read my VE table and tell me where the coverage is thin."*
   - *"Summarize the tune health for veTable1."*
   - *"What are the sensors reading right now?"*
4. It calls `list_tables`, `read_table`, `summarize_tune_context`, and so on,
   and answers from the real data.
5. To act on the advice, make the change yourself — in the table editor, or by
   asking the in-app [AI Assistant](./ai-assistant.md), which does have
   propose tools and a review queue.

## Troubleshooting

| Problem | Likely cause / fix |
|---------|-------------------|
| Client reports 401 Unauthorized | The token was regenerated since the client was configured. Copy the current one from Settings. |
| Server will not start | The port is in use. Pick another port, or free it. |
| `get_realtime_snapshot` returns an error | No live ECU connection. Connect first. |
| `read_table` says no INI definition is loaded | Open a project so a definition and tune are loaded. |
| `query_datalog` finds no entries | Start datalogging, or name a saved log from the project's datalogs folder. |
| Tools list is empty in the client | The client connected but the session dropped — reconnect. Check that LibreTune is still running with the toggle on. |
