# Keeping a project in Git

A project is a folder of small text files, so it fits well in a Git repository: on GitHub, GitLab, Gitea or any other place that speaks Git over HTTPS. That gives you the bot on several computers, a history of every version, and a way to work on it with others. **File › Git…** does it from inside XC Circuit; you don't need Git installed.

## Setting it up

1. Make a new, **empty** repository where you want the project stored. Make it private if the bot is only yours.
2. Copy its **HTTPS address** (the page shows it under *Code* or *Clone*; it starts with `https://`, not `git@`).
3. Open **File › Git…**, paste the address, and sign in:
   - **GitHub:** press **Sign in with GitHub**. A page opens in your browser; type in the code XC Circuit shows and approve. No password goes through the app.
   - **Anywhere else, or if you prefer:** make an **access token** in your account's settings there, allowed to read and write repositories (GitHub: *Developer settings › Personal access tokens*; GitLab: *Access tokens* with `write_repository`; Gitea: *Applications*), and paste it.
4. Press **Connect**.

The sign-in is kept in your system's credential store, one per place (`github.com`, your own Gitea …), for all your projects there. It is never written into the project.

## A project without a folder of its own

A project kept in a Git repository doesn't need a folder you look after. **File › New Project…** offers **In a Git repository** beside **Only on this computer**, and **Get from Git…** offers **Kept by XC Circuit**: the project then lives in the app's own folder (on Windows under `%LOCALAPPDATA%\XC Circuit\remotes`), as a complete copy you can work on without a network, and stays in your recent projects. **Show the folder** in File › Git… opens it. Choose a folder of your own instead when the project has files you reach by their path, such as pictures.

**Disconnect…** in File › Git… forgets the repository: the project and its history stay on the computer, and what is stored there stays there. **Disconnect and keep as a folder…** also moves a project out of the app's folder into one you choose.

## Store and Fetch

- **Store** saves the project, records what changed since the last time (with the few words you give it) and sends it to the repository.
- **Fetch** gets what is stored there and puts it together with what you have here. Your own changes are recorded first, so nothing of yours is lost.

When newer changes are stored there, Store says so: press **Fetch** first, then **Store** again.

## Earlier versions

Every Store is a version. **Earlier versions** in File › Git… lists the newest ones with their words, who stored them and when. **Go back to this** makes the project what it was then, as a new version on top: nothing in between is lost, and you can go back to where you were the same way.

## Before something secret is stored

When a block holds something that looks like a secret (a webhook address, a bot token, an access key), Store stops and shows where. Whoever can read the repository could read it. Keep such things in **Bot › Settings** instead; or press **Store anyway** if it is meant to be there.

## On another computer

**File › Get from Git…** (also on the start page) brings a stored project onto a computer: paste the repository's address, choose where its folder goes, sign in if the repository is private, and press **Get the project**. It opens right away. Then fill in the bot token once in **Bot › Settings**; it was never stored.

## When both changed the same thing

The project's files are put together by what they hold, not line by line: blocks by their number, wires one by one, every setting of a block on its own. So two people can work in the same workspace: one changes a block's message while the other adds blocks and wires, and both arrive. Where a block sits never stands in the way; if both moved it, yours stays. Only where both changed the **same setting** in different ways (or one deleted a block the other changed) XC Circuit **changes nothing** and lists those files. Choose for each whose version stays, **Mine** or **The stored one**, then press **Put together**. The version you didn't pick stays in the repository's history.

The two versions are put together in memory first; the project's folder only changes once the result is whole. A file that would no longer read after both changes went in counts as changed by both, and is asked about too.

## What is stored, and what never is

Stored: `circuit.project.json`, `workspaces/`, `commands/`, and whatever else you keep in the folder (the project's own plugins, pictures).

Never stored: the `.circuit/` folder. That is this computer's part: where the bot is uploaded to, database connections, private settings, the bot's stored data. And the bot token and passwords are not in the project's files at all; they stay in the credential store. On another computer you fill in the token once in **Bot › Settings**.

## Good to know

- A repository you set up yourself with Git works too: File › Git… uses it as it is, and adds `.circuit/` to its `.gitignore`.
- Files are stored byte for byte, whatever line-ending setting your own Git has, so a project is the same on Windows and Linux.
- When a project opens, XC Circuit looks whether newer versions are stored there (nothing is changed), so the status bar can say **Git: 2 to fetch**.
- The status bar shows how the project stands: **Git: stored**, or how many changes wait to be stored or fetched. Click it to open File › Git…. Store, Fetch and the dialog can each get a shortcut in Settings › Shortcuts.
