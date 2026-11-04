---
title: Download
---

# Download

XC™ Circuit runs on **Windows** (10 and 11, 64-bit) and **Linux** (x86-64).

<p>
  <a class="VPButton medium brand" href="https://github.com/XCraftTM/XC-Circuit/releases/latest" style="display:inline-block">Latest release on GitHub</a>
</p>

Download the archive for your system, unpack it anywhere and start `xc-circuit` (`xc-circuit.exe` on Windows). Nothing is installed system-wide. The app looks for newer versions itself (Help › Check for Updates; you can turn the check off in Settings).

## What you need

- **Node.js 22 or newer** to run bots. The first start finds it, or installs it for you: with winget or the official installer on Windows, as a private copy on Linux.
- The npm packages a bot needs (discord.js for Discord bots, @fluxerjs/core for Fluxer bots) are installed once, on the first run, into `%APPDATA%\XC Circuit\packages` or `~/.local/share/xc-circuit/packages`.

## What's in the download

| File or folder | What it is |
| --- | --- |
| `xc-circuit` / `xc-circuit.exe` | The editor, with the Node.js runtime that runs your block graphs built in |
| `plugins/` | The bundled plugins: Circuit Core, Discord Bot, Fluxer Bot, and Project Stats (an example native plugin) |
| `examples/` | DemoBot and the starters New Project offers |
| `README.md` | A short guide to the editor |
| `LICENSE`, `THIRD-PARTY-NOTICES.txt`, `licenses/` | The licence and the licences of what's built in |

Your own plugins go into `%APPDATA%\XC Circuit\plugins` (Windows) or `~/.local/share/xc-circuit/plugins` (Linux); Plugins › Open folder opens it.

## From source

The code is on [GitHub](https://github.com/XCraftTM/XC-Circuit). With Rust installed:

```bash
cargo run -p circuit-editor
```

The first start opens `examples/DemoBot`. See the [licence](https://github.com/XCraftTM/XC-Circuit/blob/main/LICENSE) for what you may do with the code: you can fork it to propose changes (blocks, translations, fixes) as pull requests.
