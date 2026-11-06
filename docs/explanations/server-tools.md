# Server tools

::: info In short
The blocks in **Server Tools** look after servers: run commands over SSH, see CPU, memory and disk, restart Docker containers, send console commands to game servers (RCON), check a Minecraft server or any port. SSH, RCON and Docker only run once you allow them in **Bot › Settings › Server tools**.
:::

## Blocks

| Block | What it does | Needs |
| --- | --- | --- |
| **SSH Command** | Runs a command on your server and gives what it printed, errors and exit code. | SSH in Bot › Settings, allowed |
| **Server Usage** | CPU %, memory, disk and uptime of the computer the bot runs on, or of an SSH server (Linux). | over SSH: allowed |
| **Docker** | Lists containers, starts, stops or restarts one, reads its latest log lines. | allowed; Docker on this computer, or `tcp://host:2375` |
| **RCON Command** | Sends a console command to a game server (Minecraft, Rust, ARK, CS2 …) and gives its answer. | RCON in Bot › Settings, allowed |
| **Minecraft Server** | Whether a Java or Bedrock server is up, players (and some names), version, message of the day, icon. | nothing |
| **Port Check** | Whether a server answers on a port, and how fast. | nothing |

Minecraft Server and Port Check say *offline* as an answer (with why), so a status message goes on instead of stopping.

## Safety

These blocks act on your servers, so:

- **They're off until you allow them**: Bot › Settings › Server tools › *Allow SSH, RCON and Docker*. Only turn it on for a bot whose commands you wrote yourself.
- **`${…}` in SSH Command is quoted for the shell.** `whitelist add ${name}` stays one command even if someone's name is `x; rm -rf ~`: the shell gets `'x; rm -rf ~'` as one word. Write `${…}` where a value goes, never around a whole command.
- **Use a user just for the bot** on the server, with only the rights it needs (no root), and a key rather than a password.
- **The server's key is remembered** the first time the bot connects. If it ever changes (another machine at that address), SSH Command refuses. After replacing the server on purpose, delete the stored value `hostkey:…` in the namespace `xc.server` (Stored Data view).
- Passwords, keys and the RCON password are kept in the system's password store, like the bot token.

## Servers given on the block

SSH Command, Server Usage, RCON Command and Docker can also connect to a server the block is **given**, instead of the one in Bot › Settings: set **Server** (Server Usage: *Where*) to *Given here*. Host, port and user are fields you can type or connect; the **password** and the **private key** can only be connected, from a slash command's option or a form, say. Typed into a block they'd be saved in the project file.

This needs its own switch, Bot › Settings › Server tools › *Allow servers given on blocks*, as well as *Allow SSH, RCON and Docker*. Anyone who can use such a command can make the bot connect where they say (to machines on your own network, too), so only turn it on for commands you trust them with: one only you or your staff can use.

- The password of Bot › Settings only ever goes to the server of Bot › Settings, never to one given on a block.
- SSH remembers each given server's key too, by host and port.
- A password typed into a normal message stays in the chat. Ask for it in a slash command's option or a form: only the bot sees those.

## Setting up

- **SSH:** host, port, user, and a password or a private key (the whole key text). Test with `SSH Command` running `whoami`.
- **RCON:** for Minecraft, in `server.properties`: `enable-rcon=true`, `rcon.port=25575`, `rcon.password=…`. Then host, port and password in Bot › Settings. Other servers: *Given here* (above).
- **Docker:** nothing when the bot runs on the same computer; the bot's user needs to be in the `docker` group. For Docker elsewhere: `tcp://host:2375` (only on a private network: Docker's API has no password).
