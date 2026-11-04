# Native plugins (Rust)

A native plugin is a Rust library (`.dll` on Windows, `.so` on Linux) that XC™ Circuit loads into the editor. It can do the same as an [editor script](./editor) (panels, commands, checks, save and bot events) but draws its panels with [egui](https://github.com/emilk/egui) directly and runs with the editor's full access.

::: warning Prefer editor scripts
Native plugins are off until the user allows them, must be built with exactly the same compiler and libraries as the app, and can crash it. Use them only for what a script can't do, such as custom drawing.
:::

## Example

```rust
use xc_circuit_plugin::{egui, Command, Host, Level, PanelInfo, Plugin, Problem};

#[derive(Default)]
struct Hello {
    clicks: u32,
}

impl Plugin for Hello {
    fn commands(&self) -> Vec<Command> {
        vec![Command::new("hello.greet", "Say hello").description("Writes to the Console")]
    }
    fn run_command(&mut self, _id: &str, host: &mut dyn Host) {
        host.log(Level::Info, "Hello from Rust!");
    }
    fn panels(&self) -> Vec<PanelInfo> {
        vec![PanelInfo::new("hello", "Hello")]
    }
    fn panel_ui(&mut self, _id: &str, ui: &mut egui::Ui, _host: &mut dyn Host) {
        if ui.button(format!("Clicked {} times", self.clicks)).clicked() {
            self.clicks += 1;
        }
    }
    fn check(&mut self, host: &mut dyn Host) -> Vec<Problem> {
        if host.workspaces().is_empty() { vec![Problem::new("The project has no workspaces")] } else { vec![] }
    }
}

xc_circuit_plugin::export_plugin!(Hello);
```

`plugin.toml`:

```toml
id      = "hello.native"
name    = "Hello"
version = "1.0.0"

[native]
library = "hello_native"   # hello_native.dll / libhello_native.so
```

`Cargo.toml` of the crate: `crate-type = ["cdylib"]` and a dependency on `xc-circuit-plugin` (in the repository's `crates/`). The `plugins/xc.stats` plugin is a complete example.

## The Plugin trait

Every method has a default; implement what you need.

| Method | Called |
| --- | --- |
| `commands()` | once after loading: the commands to list |
| `run_command(id, host)` | when a command is used |
| `panels()` | once after loading: the panels to offer |
| `panel_ui(id, ui, host)` | every frame a panel is shown |
| `check(host) -> Vec<Problem>` | about once a second while a project is open |
| `on_bot_event(event, host)` | for `BotEvent::Started`, `Stopped`, `Log { level, text }`, `Block { workspace, node, phase, ms, error }` |
| `on_save(host)` | after the project was saved |

## The Host trait

What the editor lets the plugin see and do. Workspaces and data are passed as JSON text in the [project format](./project-format).

| Method | Description |
| --- | --- |
| `project_name()`, `project_root()` | the open project |
| `workspaces()` | `Vec<WorkspaceInfo { key, name }>` |
| `workspace_json(key)` / `set_workspace_json(key, json)` | read or replace a workspace (undoable) |
| `active_workspace()`, `selection()` | what's open and selected |
| `reveal(workspace, node)` | select a block and centre on it |
| `block_name(block)` | a block type's display name |
| `project_data(key)` / `set_project_data(key, json)` | the project's `data` |
| `log(level, text)` | write to the Console |
| `run_command(id)` | run a command after this call |
| `bot_running()` | whether the bot runs |
| `storage_get(key)` / `storage_set(key, value)` | the plugin's own store |

## Building and loading

- **Same build as the editor.** The plugin shares egui and Rust types with the editor, so both must come from the same compiler, crate versions and profile. Add the crate to the workspace and build it with `cargo plugin <crate>` (add `--release` for a release editor). Before loading, XC Circuit compares a fingerprint and refuses a plugin built differently, saying what differs.
- **Allowed by the user.** Plugins › Manage Plugins › *Allow native plugins*; plugins outside the app's own `plugins/` folder must also be allowed one by one.
- **Where the library is looked for:** the plugin folder, its `native/` folder, then next to the editor program.
- **Rebuilds reload.** The editor loads a copy of the library and swaps in a rebuilt one while it runs. A plugin that panics is switched off until it's rebuilt.
