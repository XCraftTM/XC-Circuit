# Known issues

XC™ Circuit is in **Early Access**: it works and is used for real bots, but some things are rough or untested. If you hit something not listed here, please [report it](https://github.com/XCraftTM/XC-Circuit/issues/new?template=bug_report.yml) (or Help › Report a Bug).

## Installing

- **Windows shows "Windows protected your PC"** when you start the installer, because it isn't signed yet. Choose **More info › Run anyway**.
- **Windows on ARM (ARM64)** builds are made but haven't been tried on ARM hardware yet. The x64 version also runs on ARM devices, a bit slower.
- **Linux ARM64** packages come once the repository is public.
- **The AppImage** uses your system's GTK 3, ALSA and D-Bus libraries; they're on almost every desktop Linux.
- **macOS** isn't supported yet.

## Bots

- **Exported bots** (Bot › Export) are covered by automated tests but haven't been tried widely yet. Please report anything that runs differently from the editor.
- **Fluxer bots** have no slash commands, buttons or menus, because Fluxer doesn't offer them yet. Commands are text commands such as `!help`.
- **Running bots needs Node.js 22 or newer.** The first start finds it or installs it; behind strict company networks this can fail. Install Node.js yourself then.
- Bot tokens are kept in the system's password store. On Linux that needs a running Secret Service (GNOME Keyring or KWallet).

## Editor

- **Blocks are in English only**, by design; the interface is in English and German.
- **Copy, cut and paste** use the system's keys and can't be changed in Settings › Shortcuts.
- **Native plugins** must be built with exactly the same version of XC Circuit; after an update they have to be rebuilt.
- **Project files may still change before 1.0.** Older projects are upgraded automatically when they open, and every save keeps a backup.
