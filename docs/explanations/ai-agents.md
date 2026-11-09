# AI agents (MCP)

AI agents such as Claude can work on your bot in XC Circuit: look at your workspaces, add blocks and wire them up, fix problems, and start the bot to try it. They do it through XC Circuit's **MCP server** ([Model Context Protocol](https://modelcontextprotocol.io)), which comes with the app in the **AI agents (MCP)** plugin.

## Add it to your AI client

Open **Plugins › AI agents panel**. For each AI client found on your computer there's a button:

| Client | What **Add** does | Then |
| --- | --- | --- |
| Claude Desktop | adds XC Circuit to `claude_desktop_config.json` | quit and reopen Claude Desktop |
| Claude Code | runs `claude mcp add --scope user xc-circuit …` | start a new Claude Code session |
| Cursor | adds it to `~/.cursor/mcp.json` | restart Cursor |
| VS Code | adds it to your user `mcp.json` | it shows under MCP servers in the Chat view |

For any other MCP client, **Copy settings** copies the entry to paste into its MCP settings:

```json
{
  "mcpServers": {
    "xc-circuit": { "command": "C:\\…\\XC Circuit\\xc-circuit.exe", "args": ["--mcp"] }
  }
}
```

The panel shows **Added, but for another copy of XC Circuit** when the entry points to a different `xc-circuit` (after moving the app, say): **Update** points it here. **Remove** takes it out again. Before changing a settings file for the first time, XC Circuit keeps a copy next to it (`….json.before-xc-circuit`).

## Use it

Keep XC Circuit open with your project, and ask the agent, for example: *"Add a /ping command that answers with the bot's ping"*. If XC Circuit isn't open, the agent sees one tool, `open_xc_circuit`, which opens it. When XC Circuit opens or closes later, the server tells the AI client that its tools changed, so the client picks up the full set (or loses it) by itself, without reconnecting.

What the agent can do:

| Tool | |
| --- | --- |
| `get_project` | the project, its workspaces, problems and slash commands |
| `get_workspace` · `get_block` | a workspace's blocks and wires · one block's values and pins |
| `find_block_types` · `get_block_type` | the installed blocks and their fields |
| `add_block` · `set_block_values` · `move_block` · `delete_blocks` | change blocks |
| `connect` · `disconnect` | wires, checked like ones you draw |
| `create_workspace` · `get_slash_commands` · `set_slash_commands` | workspaces and commands |
| `add_note` · `set_note` · `delete_note` | notes on the canvas, also framing blocks you name |
| `open_builder` | opens the Embed, Layout or Canvas Builder (or Slash Commands) on a block, to show you |
| `screenshot` | a picture of the canvas (a workspace fitted to view, or centred on a block, at a zoom it picks if it likes), so the agent sees what it built |
| `get_problems` · `get_console` | what's wrong, and what the bot logged |
| `list_block_types` · `get_block_source` · `list_plugins` | every block, a block's code (to see exactly what it does), the plugins |
| `get_app_settings` · `get_bot_settings` · `set_bot_settings` | XC Circuit's settings (to read), and Bot › Settings (to read and change; never the token) |
| `get_server` · `server_action` · `set_run_target` | the bot's server: its state, the last upload and its console; upload, start, restart, stop, kill; and whether Run acts here or there |
| `run_bot` · `stop_bot` · `save_project` · `show_in_editor` | run it (here or on the server), save it, show you a block |
| `run_script` | many of these at once: JavaScript that gets every tool as a function (`tools.add_block({ … })`), the [plugin API](/api/editor) as `xc`, and `print()` |

Embeds, layouts and canvases are what Build Embed, Build Layout and Build Canvas keep in a field: agents fill them with `set_block_values`, and `get_block_type` tells them the format. Embeds and layouts can also be given as Discord's own JSON, as in the builders' JSON tab.

`run_script` lets an agent build a whole flow in one step instead of a call per block. It runs in the same sandbox as plugins: no files, no network, at most 5 seconds.

Every tool the agent uses is written to the **Console** as an **MCP** line ("AI agent: add_block · workspace main, type "core.log""), with the reason when it failed and **Show block** when it's about one. The **MCP** chip above the Console shows only those. Every change shows in the editor right away and can be undone with **Ctrl+Z**, like your own. The agent never sees the bot's token.

## How it works

The AI client starts `xc-circuit --mcp`, which speaks MCP over its input and output. It passes the agent's requests to the running app through a connection on your computer only (`127.0.0.1`, with a secret written to XC Circuit's data folder, in a file only you can read). A program that connects without the secret is let go. The app answers them with the plugins' tools: plugins can add their own with [`xc.tool`](/api/editor#xc-tool).

## Turn it off

Under **Plugins**, switch **AI agents (MCP)** off: the server stops listening and agents see only `open_xc_circuit`. The trash can removes it; it comes with the app, so **Add back** at the end of the Plugins list brings it back.
