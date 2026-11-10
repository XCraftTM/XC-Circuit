# The Host: a place of your own

The **XC Circuit Host** is a small server you run yourself, in a Docker container. It is for people who want their projects on a machine of their own instead of GitHub, and it is what live sessions go through.

::: warning Work in progress
The container is here and tested, and from XC Circuit you can pair with it, keep projects on it and carry a project's secrets between your devices. [Live sessions](live.md) can meet at your host instead of the public relay. Running the bot on the host is still being built.
:::

## What it does

| | What it keeps |
| --- | --- |
| **Projects** | Your projects with every version, as Git repositories: XC Circuit keeps in step with it the same way as with GitHub. |
| **Vault** | Your bots' secrets (token, passwords), **encrypted by your devices**. The host can't read them, and neither can someone who gets hold of its data. |
| **Relay** | Nothing: it passes the messages of a live session from one app to the others. |

It has two modes, set with `XC_HOST_MODE`:

- **`full`** (the default): all three. It needs a data folder (a volume); without one it doesn't start, and says so, instead of writing into the container where an update would wipe it.
- **`relay`**: only the relay. Nothing is kept, no volume is needed. This is what a public relay runs.

## Setting it up

You need a server with Docker and a reverse proxy that adds HTTPS, such as Nginx Proxy Manager.

1. Get [`host/docker-compose.yml`](https://github.com/XCraftTM/XC-Circuit/blob/main/host/docker-compose.yml) and start it. It uses the published image, `ghcr.io/xcrafttm/xc-circuit-host` (for x64 and ARM); in Portainer, a stack with the same file does it:

   ```bash
   docker compose -f host/docker-compose.yml up -d
   ```

   The compose file puts the host on your proxy's Docker network (change the network's name to yours) and publishes **no port**: it is only reachable through the proxy.

2. In the proxy, add a host for your domain (say `circuit.example.com`) that forwards to `http://xc-circuit-host:7421`, with **Websockets Support** on, a certificate, and **Force SSL**.

3. Open `https://circuit.example.com/status`. It answers with its mode and protocol version:

   ```json
   { "name": "xc-circuit-host", "version": "0.3.0", "protocol": 1, "mode": "full", "can": ["relay", "projects", "vault"] }
   ```

4. Pair your first device. There is no password to start with: the host prints a code in its log, good for ten minutes (restart it for a new one):

   ```bash
   docker compose -f host/docker-compose.yml logs xc-circuit-host
   ```

   In XC Circuit, open **File › Host…**, fill in the host's address and that code, and press **Pair**.

## Your devices

The first device makes the **key your secrets are sealed with**, and shows it once as a **recovery code**: write it down. Your devices pass the key on to each other; if all of them are ever gone, the recovery code is the only way to read the secrets on the host again (tick *I have a recovery code* when pairing the next first device).

Every further device comes in through one that is paired already:

1. On the paired device: **File › Host… › Add a device**. It shows a code of 16 letters and digits. Leave that window open.
2. On the new device: **File › Host…**, the host's address and that code, **Pair**.

Half of the code goes to the host, which lets the device in. The other half only travels from one screen to the other keyboard: the two devices use it to pass the key on, so the host can't get at it even if it tried.

**Unpair this computer…** takes it off the host's list and deletes its token and its key here.

## A project on the host

With a project open, File › Host… has three buttons:

- **Keep this project on the host** stores it there as a Git repository and keeps it in step from then on, exactly as [with GitHub](git.md); File › Git… opens to show it.
- **Send its secrets** seals the bot token and the project's other secrets on this computer and puts them in the host's vault.
- **Get its secrets** fetches them on another device and saves them in that computer's credential store. No more typing the token in on every device.

The passwords for uploading the bot from one computer (Bot › Remote…) don't travel: they belong to that computer.

On another device, get the project with **File › Get from Git…** and the address `https://circuit.example.com/git/my-bot.git`; once paired, that device is signed in there already.

## Settings

| Variable | Default | |
| --- | --- | --- |
| `XC_HOST_MODE` | `full` | `full` or `relay`. |
| `XC_HOST_PORT` | `7421` | The port inside the container. Not a port of any common service, and never open to the internet: the proxy is. |
| `XC_HOST_DATA` | `/data` | Where full mode keeps everything: `host.sqlite` and `projects/`. |
| `XC_HOST_TRUSTED_PROXY` | (none) | Whose word to take for an app's address (`X-Forwarded-For`), to limit misuse per person: `private` for a proxy on a private network, or the proxy's addresses, separated by commas. |

## Good to know

- **Backups are yours to make.** Everything is in the data volume: copy it while the container is stopped, or with a snapshot.
- **The proxy sees the traffic**, as it adds the HTTPS. That is fine: secrets are encrypted by your devices before they are sent, and live sessions are encrypted between the apps.
- **Limits** keep one person from filling it up: 16 guests in a session, 4 sessions and 32 connections per address, 256 KB per message, 64 KB of secrets per project. A code for pairing runs out after 10 minutes; a key nobody fetched, after 30 days.
- The host and the app are updated on their own, so they tell each other their **protocol version** first.
- A sign of life goes both ways every 30 seconds, as proxies cut connections that say nothing (Nginx after 60 seconds).

## For developers: the messages

Everything but Git goes over one WebSocket at `/ws`, as JSON with a `t`:

| From the app | Answer | |
| --- | --- | --- |
| `open` | `opened { room }` | Starts a live session. Its host owns it: when they leave, it ends (`closed`). |
| `join { room }` | `joined { you }` | The session's host gets `peer { id, on }`. |
| `to { data, id? }` | | A guest's goes to the host; the host's to one guest (`id`) or all. Arrives as `from { id, data }`. `data` is the apps' own, encrypted by them. |
| `kick { id }`, `leave`, `ping` | `pong` | |
| `pair { code, name, key }` | `paired { device, token }` | Full mode. `key` is the device's public key, for the others to wrap the vault's key for it. |
| `hello { token }` | `welcome { device, name }` | Says who this is. Needed for all below. |
| `invite` | `invited { code, minutes }` | A code for one more device. |
| `devices`, `device.remove { id }` | `devices { devices, you }` | |
| `vault.get { project }` | `vault { blob, rev }` | |
| `vault.put { project, blob, rev }` | `vault.kept { rev }` | `rev` is the one last seen: a newer one there refuses it. |
| `key.put { for, blob }`, `key.get` | `keys { keys }` | A key wrapped for another device, kept until it fetches it. |

What goes wrong comes back as `error { re, why }`. The source is one file, [`host/server.js`](https://github.com/XCraftTM/XC-Circuit/blob/main/host/server.js), with no dependencies beyond Node.js 24 and Git; `node host/test/host.js` runs its tests.
