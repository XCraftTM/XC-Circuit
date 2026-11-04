# Security

If you find a security problem in XC™ Circuit (for example a way for a plugin, a project file or a website to reach things it shouldn't, or a bot token ending up somewhere it shouldn't), please **report it privately**, not as a public issue:

**[Report a vulnerability](https://github.com/XCraftTM/XC-Circuit/security/advisories/new)** (GitHub's private reporting).

Please include what you found, how to reproduce it, and which version you used (Help › About). You'll get an answer as soon as possible. Once it's fixed, you'll be credited in the changelog if you like.

Only the latest release gets security fixes while XC Circuit is in Early Access.

## What XC Circuit does to keep you safe

- **Bot tokens** are kept in Windows Credential Manager or the Linux Secret Service, never in project files.
- **Editor scripts** (plugins' `editor.js`) run in a sandbox with time and memory limits and only reach the app through the `xc` API: no files, network or programs.
- **Native plugins** (compiled Rust) are off until you allow them, and plugins outside the app's own folder must be allowed one by one.
- **Installing a plugin from a zip** can't write outside the plugin's own folder.
- **No telemetry.** XC Circuit only goes online to check GitHub for updates (which you can turn off in Settings) and when your bot runs.
