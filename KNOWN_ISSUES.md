# Known issues

XC™ Circuit is in **Early Access**: it works and is used for real bots, but some things are rough or untested. If you hit something not listed here, please [report it](https://github.com/XCraftTM/XC-Circuit/issues/new?template=bug_report.yml) (or Help › Report a Bug).

## Installing

- **Windows shows "Windows protected your PC"** when you start the installer, because it isn't signed yet. Choose **More info › Run anyway**.
- **Windows on ARM (ARM64)** builds are made but haven't been tried on ARM hardware yet. The x64 version also runs on ARM devices, a bit slower.
- **Linux ARM64** packages are built and pass their tests on ARM machines, but haven't been tried on a real device (such as a Raspberry Pi) yet.
- **The AppImage** uses your system's GTK 3, ALSA and D-Bus libraries; they're on almost every desktop Linux.
- **macOS** isn't supported yet.

## Bots

- **Exported bots** (Bot › Export) are tested, but if one runs differently from the editor, please report it.
- **Fluxer bots** have no slash commands, buttons or menus, because Fluxer doesn't offer them yet. Commands are text commands such as `!help`.
- **Running bots needs Node.js 22 or newer.** The first start finds it or installs it; behind strict company networks this can fail. Install Node.js yourself then.
- Bot tokens are kept in the system's password store. On Linux that needs a running Secret Service (GNOME Keyring or KWallet).

## Editor

- **Blocks are in English only**, by design; the interface is in English and German.
- **On some computers XC Circuit stops answering now and then** (Windows says "Not Responding"), and it isn't clear yet why. Programs that read windows (some password managers and writing helpers) are one suspect: if it happens to you, try turning off Settings › Window › Accessibility, new after 0.2.7 (screen readers and the touch keyboard then can't see the window). Either way, `freezes.log` in the app's data folder says where it was stuck; please send it along when you report it.
- **Copy, cut and paste** use the system's keys and can't be changed in Settings › Shortcuts.
- **Project files may still change before 1.0.** Older projects are upgraded automatically when they open, and every save keeps a backup.
