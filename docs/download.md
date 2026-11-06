---
title: Download
---

# Download

::: info Early Access
XC™ Circuit is in Early Access: it works and is used for real bots, but expect rough edges. See the [known issues](/known-issues) and [what's new](/changelog). It's [built with AI assistance](/made-with-ai).
:::

<Downloads />

## After downloading

- **Windows Setup**: run it. It installs for you only; no administrator needed.
- **Windows .zip** or **Linux .tar.gz**: unpack it anywhere and start `xc-circuit.exe` (Windows) or `xc-circuit` (Linux).
- **AppImage**: make it executable (right-click › Properties › *Allow executing*, or `chmod +x`), then double-click it.
- **.deb**: `sudo apt install ./XC-Circuit-…deb` · **.rpm**: `sudo dnf install ./XC-Circuit-…rpm`

Not sure whether your computer is x64 or ARM64? Almost every PC is x64; ARM64 is mostly laptops with a Snapdragon chip.

::: tip "Windows protected your PC"?
The installer isn't signed yet, so Windows asks first. Choose **More info › Run anyway**.
:::

## Updates

The app looks for newer versions itself (at start, and from Help › Check for Updates; you can turn the check off in Settings).

- **Installed with the Windows Setup, or an AppImage:** **Update and restart** downloads the new version, checks it against the checksum GitHub publishes, and installs it as XC Circuit closes (asking about unsaved changes first). Then XC Circuit opens again. On Windows only a small progress window shows; installed for everyone, Windows asks for administrator rights.
- **.deb, .rpm, .zip or .tar.gz:** **Download** opens the release page; install the new file as you did the first one.

## What you need

- **Node.js 22 or newer** to run bots. The first start finds it, or installs it for you: with winget or the official installer on Windows, as a private copy on Linux.
- The npm packages a bot needs (discord.js for Discord bots, @fluxerjs/core for Fluxer bots) are installed once, on the first run, into `%APPDATA%\XC Circuit\packages` or `~/.local/share/xc-circuit/packages`.

## What's in the download

| File or folder | What it is |
| --- | --- |
| `xc-circuit` / `xc-circuit.exe` | The editor, with the Node.js runtime that runs your block graphs built in |
| `plugins/` | The bundled plugins: Circuit Core, Discord Bot, Fluxer Bot and Server Tools |
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
