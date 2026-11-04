# Templates

Templates are the starters File › New Project offers: working bots people begin from. A plugin adds them by putting project folders into its `templates/` folder; they don't need to be listed in `plugin.toml`.

```
acme.chat/
└─ templates/
   └─ basics/
      ├─ template.json
      ├─ circuit.json
      └─ workspaces/
         ├─ ping.json
         └─ help.json
```

Each template is an ordinary [project](./project-format) with a `template.json`:

```json
{
  "name": "Basic commands",
  "description": "!ping shows how fast the bot answers, !help lists the commands.",
  "mode": "fluxer-bot"
}
```

| Key | Description |
| --- | --- |
| `name` | Shown in the list of starters. |
| `description` | One or two sentences: what the bot does and what to set up. |
| `mode` | The [build mode](./build-modes) it's for. Default: the `mode` in its `circuit.json`. New Project only offers it for that mode. |

Creating a project from a template copies its `circuit.json` and workspaces (other files, such as a `plugins/` folder, are not copied), gives it the name typed in New Project, and adds a `.gitignore` for `.circuit/`. The app's own starters are in `examples/templates/`.

Tips:

- Say in the description what has to be filled in (a role id, a channel).
- Keep templates small and readable: people learn from them.
