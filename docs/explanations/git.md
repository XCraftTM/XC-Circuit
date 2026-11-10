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

## Store and Fetch

- **Store** saves the project, records what changed since the last time (with the few words you give it) and sends it to the repository.
- **Fetch** gets what is stored there and puts it together with what you have here. Your own changes are recorded first, so nothing of yours is lost.

When newer changes are stored there, Store says so: press **Fetch** first, then **Store** again.

## When both changed the same thing

Changes to different files, or to different parts of one file, are put together without asking. Where both sides changed the same part of a file, XC Circuit **changes nothing** and lists those files. Choose for each whose version stays, **Mine** or **The stored one**, then press **Put together**. The version you didn't pick stays in the repository's history.

The two versions are put together in memory first; the project's folder only changes once the result is whole. A file that would no longer read after both changes went in counts as changed by both, and is asked about too.

## What is stored, and what never is

Stored: `circuit.project.json`, `workspaces/`, `commands/`, and whatever else you keep in the folder (the project's own plugins, pictures).

Never stored: the `.circuit/` folder. That is this computer's part: where the bot is uploaded to, database connections, private settings, the bot's stored data. And the bot token and passwords are not in the project's files at all; they stay in the credential store. On another computer you fill in the token once in **Bot › Settings**.

## Good to know

- A repository you set up yourself with Git works too: File › Git… uses it as it is, and adds `.circuit/` to its `.gitignore`.
- Files are stored byte for byte, whatever line-ending setting your own Git has, so a project is the same on Windows and Linux.
- To get a stored project onto another computer today, clone it with Git or download it from the repository's page, then open the folder.
