//! Keeping a project in a Git repository, to have it on several computers
//! and to work on it with others: GitHub, GitLab, Gitea or any other place
//! that speaks Git over HTTPS.
//!
//! The app shows two actions. **Store** records what changed here and sends
//! it; **Fetch** gets what others stored and puts the two together. Nothing
//! here leaves the project half merged: two versions are put together in
//! memory first, and the folder only changes once the result is whole. Where
//! both sides changed the same file (or putting them together gave a file
//! that no longer reads), the caller says which side's file to keep.

pub mod github;

use anyhow::{Context, Result, anyhow, bail};
use git2::{Cred, FetchOptions, IndexEntry, IndexTime, Oid, PushOptions, RemoteCallbacks, Repository, Signature, StatusOptions, Tree, build::CheckoutBuilder};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// The name the place a project is stored at has in its repository.
const REMOTE: &str = "origin";
/// What a project's repository never holds: this computer's part.
const IGNORED: &[&str] = [".circuit/"].as_slice();

/// Who signs in to the place a project is stored at. Kept in the OS
/// credential store by the app, never in a file.
#[derive(Clone, Default)]
pub struct Login {
    /// The account's name. Most places take any name beside a token.
    pub user: String,
    /// An access token (or a password, where a place still takes one).
    pub token: String,
}

/// Whose name stands on what is stored.
#[derive(Clone, Debug)]
pub struct Who {
    pub name: String,
    pub email: String,
}

/// Which version of a file to keep where both sides changed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// This computer's.
    Mine,
    /// The stored one's.
    Theirs,
}

/// How a project stands against where it is stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// Where it is stored, if anywhere yet.
    pub remote: Option<String>,
    pub branch: String,
    /// The files changed here since the last Store.
    pub changed: Vec<String>,
    /// Stored here, not sent yet.
    pub ahead: usize,
    /// Fetched, not put together with this computer's yet. Only as new as
    /// the last [`Sync::fetch`].
    pub behind: usize,
}

/// What putting the fetched version together with this one did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Merged {
    /// Nothing new was there.
    UpToDate,
    /// The project is now what was stored, with this computer's changes in
    /// it: the files that changed here.
    Updated(Vec<String>),
    /// Both sides changed these files (or together they no longer read):
    /// nothing was changed. Call again with a [`Side`] for each.
    Conflicts(Vec<String>),
}

/// One file on both sides; `None` where a side has no such file.
pub struct Versions {
    pub mine: Option<Vec<u8>>,
    pub theirs: Option<Vec<u8>>,
}

/// A project's repository.
pub struct Sync {
    repo: Repository,
}

impl Sync {
    /// The repository of the project in `root`, if it has one. Only its own:
    /// a project lying somewhere inside another repository has none.
    pub fn open(root: &Path) -> Result<Option<Sync>> {
        match Repository::open(root) {
            Ok(repo) => Ok(Some(Sync { repo })),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("opening the repository in {}", root.display())),
        }
    }

    /// Opens the project's repository, making one when it has none.
    pub fn start(root: &Path) -> Result<Sync> {
        let sync = match Sync::open(root)? {
            Some(sync) => sync,
            None => {
                let mut options = git2::RepositoryInitOptions::new();
                options.initial_head("main");
                Sync { repo: Repository::init_opts(root, &options).with_context(|| format!("making a repository in {}", root.display()))? }
            }
        };
        keep_ignored(root)?;
        sync.keep_bytes()?;
        Ok(sync)
    }

    /// Gets a stored project into the new folder `root`.
    pub fn get(url: &str, root: &Path, login: &Login) -> Result<Sync> {
        let mut fetch = FetchOptions::new();
        fetch.remote_callbacks(callbacks(login));
        let repo = git2::build::RepoBuilder::new().fetch_options(fetch).clone(url, root).map_err(|e| explain(e, "getting the project"))?;
        let sync = Sync { repo };
        sync.keep_bytes()?;
        if sync.tip().is_some() {
            sync.checkout()?;
        }
        keep_ignored(root)?;
        Ok(sync)
    }

    /// Files go in and out byte for byte, whatever this computer's Git is
    /// set to (Git for Windows turns line ends around by default): a
    /// project's files are the same on every computer.
    fn keep_bytes(&self) -> Result<()> {
        let mut config = self.repo.config()?.open_level(git2::ConfigLevel::Local)?;
        config.set_bool("core.autocrlf", false)?;
        config.set_bool("core.safecrlf", false)?;
        Ok(())
    }

    /// Sets where the project is stored.
    pub fn set_remote(&self, url: &str) -> Result<()> {
        let url = url.trim();
        if url.is_empty() {
            bail!("Fill in the repository's address.");
        }
        match self.repo.find_remote(REMOTE) {
            Ok(_) => self.repo.remote_set_url(REMOTE, url)?,
            Err(_) => drop(self.repo.remote(REMOTE, url)?),
        }
        Ok(())
    }

    fn branch(&self) -> Result<String> {
        // HEAD names a branch even before anything was stored on it.
        let head = self.repo.find_reference("HEAD")?;
        let name = head.symbolic_target().ok().flatten().ok_or_else(|| anyhow!("The repository isn't on a branch (a detached HEAD): switch to one with Git first."))?;
        Ok(name.trim_start_matches("refs/heads/").to_string())
    }

    fn tip(&self) -> Option<git2::Commit<'_>> {
        self.repo.head().ok().and_then(|h| h.peel_to_commit().ok())
    }

    fn stored_tip(&self) -> Result<Option<git2::Commit<'_>>> {
        let name = format!("refs/remotes/{REMOTE}/{}", self.branch()?);
        Ok(self.repo.find_reference(&name).ok().and_then(|r| r.peel_to_commit().ok()))
    }

    pub fn state(&self) -> Result<State> {
        let mut options = StatusOptions::new();
        options.include_untracked(true).recurse_untracked_dirs(true);
        let mut changed: Vec<String> = self.repo.statuses(Some(&mut options))?.iter().filter_map(|s| s.path().ok().map(str::to_string)).collect();
        changed.sort();
        let (mut ahead, mut behind) = (0, 0);
        match (self.tip(), self.stored_tip()?) {
            (Some(mine), Some(theirs)) => (ahead, behind) = self.repo.graph_ahead_behind(mine.id(), theirs.id())?,
            (Some(mine), None) => {
                let mut walk = self.repo.revwalk()?;
                walk.push(mine.id())?;
                ahead = walk.count();
            }
            (None, Some(theirs)) => {
                let mut walk = self.repo.revwalk()?;
                walk.push(theirs.id())?;
                behind = walk.count();
            }
            (None, None) => {}
        }
        Ok(State { remote: self.repo.find_remote(REMOTE).ok().and_then(|r| r.url().ok().map(str::to_string)), branch: self.branch()?, changed, ahead, behind })
    }

    /// Records the project as it is now. False when nothing changed since
    /// the last time.
    pub fn store(&self, message: &str, who: &Who) -> Result<bool> {
        let mut index = self.repo.index()?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
        index.update_all(["*"], None)?;
        index.write()?;
        let tree = self.repo.find_tree(index.write_tree()?)?;
        let tip = self.tip();
        if tip.as_ref().is_some_and(|t| t.tree_id() == tree.id()) || (tip.is_none() && tree.is_empty()) {
            return Ok(false);
        }
        let sig = signature(who)?;
        let parents: Vec<&git2::Commit> = tip.iter().collect();
        self.repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
        Ok(true)
    }

    /// Sends what was stored here. Refused when newer changes are stored
    /// there: fetch and put them together first.
    pub fn send(&self, login: &Login) -> Result<()> {
        let branch = self.branch()?;
        let mut remote = self.repo.find_remote(REMOTE).map_err(|_| anyhow!("The project isn't stored anywhere yet: set its repository first."))?;
        let refused = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
        let mut cb = callbacks(login);
        let seen = refused.clone();
        cb.push_update_reference(move |_, status| {
            if let Some(why) = status {
                *seen.borrow_mut() = Some(why.to_string());
            }
            Ok(())
        });
        let mut options = PushOptions::new();
        options.remote_callbacks(cb);
        let spec = format!("refs/heads/{branch}:refs/heads/{branch}");
        remote.push(&[spec.as_str()], Some(&mut options)).map_err(|e| if e.code() == git2::ErrorCode::NotFastForward { anyhow!(NEWER_THERE) } else { explain(e, "sending the project") })?;
        if let Some(why) = refused.borrow().as_ref() {
            bail!("The repository didn't take it: {why}");
        }
        // What is stored there is now this.
        if let Some(tip) = self.tip() {
            self.repo.reference(&format!("refs/remotes/{REMOTE}/{branch}"), tip.id(), true, "sent")?;
        }
        Ok(())
    }

    /// Gets what is stored, without changing the project yet.
    pub fn fetch(&self, login: &Login) -> Result<()> {
        let mut remote = self.repo.find_remote(REMOTE).map_err(|_| anyhow!("The project isn't stored anywhere yet: set its repository first."))?;
        let mut options = FetchOptions::new();
        options.remote_callbacks(callbacks(login));
        let spec = format!("+refs/heads/*:refs/remotes/{REMOTE}/*");
        remote.fetch(&[spec.as_str()], Some(&mut options), None).map_err(|e| explain(e, "fetching the project"))?;
        Ok(())
    }

    /// Puts the fetched version together with this computer's. What changed
    /// here must be stored first ([`Sync::store`]). `keep` says which side's
    /// file to keep for each file both changed.
    pub fn merge(&self, keep: &HashMap<String, Side>, who: &Who) -> Result<Merged> {
        if !self.state()?.changed.is_empty() {
            bail!("Store what changed here first.");
        }
        let Some(theirs) = self.stored_tip()? else { return Ok(Merged::UpToDate) };
        let branch = format!("refs/heads/{}", self.branch()?);
        let Some(mine) = self.tip() else {
            // Nothing here yet: the project becomes what is stored.
            self.repo.reference(&branch, theirs.id(), true, "fetched")?;
            self.checkout()?;
            return Ok(Merged::Updated(paths_of(&self.repo, None, &theirs.tree()?)?));
        };
        if mine.id() == theirs.id() || self.repo.graph_descendant_of(mine.id(), theirs.id())? {
            return Ok(Merged::UpToDate);
        }
        let before = mine.tree()?;
        if self.repo.graph_descendant_of(theirs.id(), mine.id())? {
            // Only they changed something: take theirs as it is.
            self.repo.reference(&branch, theirs.id(), true, "fetched")?;
            self.checkout()?;
            return Ok(Merged::Updated(paths_of(&self.repo, Some(&before), &theirs.tree()?)?));
        }
        // Both changed something: put them together in memory.
        let mut index = self.repo.merge_commits(&mine, &theirs, None)?;
        let mut unsure: BTreeSet<String> = BTreeSet::new();
        for conflict in index.conflicts()? {
            let c = conflict?;
            if let Some(path) = [c.our, c.their, c.ancestor].into_iter().flatten().next().and_then(|e| String::from_utf8(e.path).ok()) {
                unsure.insert(path);
            }
        }
        // A JSON file put together from both that no longer reads is one too.
        let (my_tree, their_tree) = (mine.tree()?, theirs.tree()?);
        for entry in index.iter().filter(|e| (e.flags >> 12) & 3 == 0) {
            let Ok(path) = String::from_utf8(entry.path.clone()) else { continue };
            let same = |tree: &Tree| tree.get_path(Path::new(&path)).is_ok_and(|e| e.id() == entry.id);
            if path.ends_with(".json") && !same(&my_tree) && !same(&their_tree) && self.repo.find_blob(entry.id).is_ok_and(|b| serde_json::from_slice::<serde_json::Value>(b.content()).is_err()) {
                unsure.insert(path);
            }
        }
        let open: Vec<String> = unsure.iter().filter(|p| !keep.contains_key(*p)).cloned().collect();
        if !open.is_empty() {
            return Ok(Merged::Conflicts(open));
        }
        for path in &unsure {
            let from = if keep[path] == Side::Mine { &my_tree } else { &their_tree };
            index.conflict_remove(Path::new(path)).ok();
            index.remove_path(Path::new(path)).ok();
            // A side that deleted the file: it stays deleted.
            if let Ok(e) = from.get_path(Path::new(path)) {
                index.add(&IndexEntry { ctime: IndexTime::new(0, 0), mtime: IndexTime::new(0, 0), dev: 0, ino: 0, mode: e.filemode() as u32, uid: 0, gid: 0, file_size: 0, id: e.id(), flags: 0, flags_extended: 0, path: path.as_bytes().to_vec() })?;
            }
        }
        let tree = self.repo.find_tree(index.write_tree_to(&self.repo)?)?;
        let sig = signature(who)?;
        self.repo.commit(Some("HEAD"), &sig, &sig, "Put together with what was stored", &tree, &[&mine, &theirs])?;
        self.checkout()?;
        Ok(Merged::Updated(paths_of(&self.repo, Some(&before), &tree)?))
    }

    /// Makes the folder what the branch's newest version says. Only called
    /// with everything here stored, so nothing of the user's is lost.
    fn checkout(&self) -> Result<()> {
        self.repo.checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(false)))?;
        Ok(())
    }

    /// A file as it is stored there and as it was stored here last, to show
    /// both where they differ.
    pub fn versions(&self, path: &str) -> Result<Versions> {
        let read = |commit: Option<git2::Commit>| -> Option<Vec<u8>> {
            let entry = commit?.tree().ok()?.get_path(Path::new(path)).ok()?;
            Some(self.repo.find_blob(entry.id()).ok()?.content().to_vec())
        };
        Ok(Versions { mine: read(self.tip()), theirs: read(self.stored_tip()?) })
    }
}

/// What a Send is told when newer changes are stored there.
pub const NEWER_THERE: &str = "Newer changes are stored there: fetch them first.";

fn signature(who: &Who) -> Result<Signature<'static>> {
    let name = if who.name.trim().is_empty() { "XC Circuit" } else { who.name.trim() };
    let email = if who.email.trim().is_empty() { "xc-circuit@localhost" } else { who.email.trim() };
    Ok(Signature::now(name, email)?)
}

/// The files that differ between two versions.
fn paths_of(repo: &Repository, before: Option<&Tree>, after: &Tree) -> Result<Vec<String>> {
    let diff = repo.diff_tree_to_tree(before, Some(after), None)?;
    let mut paths: Vec<String> = diff.deltas().filter_map(|d| d.new_file().path().or(d.old_file().path()).map(|p| p.to_string_lossy().replace('\\', "/"))).collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Signing in with the login, once: a place that refuses it is not asked
/// again and again.
fn callbacks(login: &Login) -> RemoteCallbacks<'static> {
    let login = login.clone();
    let mut tried = false;
    let mut cb = RemoteCallbacks::new();
    cb.credentials(move |_url, from_url, _allowed| {
        if tried || login.token.is_empty() {
            return Err(git2::Error::from_str("sign-in refused"));
        }
        tried = true;
        let user = if !login.user.is_empty() { login.user.as_str() } else { from_url.unwrap_or("git") };
        Cred::userpass_plaintext(user, &login.token)
    });
    cb
}

/// A Git error in the user's words, without the address's sign-in part.
fn explain(e: git2::Error, doing: &str) -> anyhow::Error {
    let said = e.message().to_string();
    match (e.code(), e.class()) {
        (git2::ErrorCode::Auth, _) | (_, git2::ErrorClass::Http) if said.contains("sign-in refused") || said.contains("401") || said.contains("403") || e.code() == git2::ErrorCode::Auth => {
            anyhow!("Signing in didn't work while {doing}: check the access token, and that it may read and write this repository.")
        }
        (git2::ErrorCode::NotFound, _) => anyhow!("There is no repository at that address (or the sign-in can't see it)."),
        _ => anyhow!("{doing} didn't work: {said}"),
    }
}

/// Makes sure the project's `.gitignore` keeps this computer's part out,
/// leaving what the user added to it.
fn keep_ignored(root: &Path) -> Result<()> {
    let file = root.join(".gitignore");
    let mut text = std::fs::read_to_string(&file).unwrap_or_default();
    let missing: Vec<&str> = IGNORED.iter().copied().filter(|line| !text.lines().any(|l| l.trim() == *line || l.trim() == line.trim_end_matches('/'))).collect();
    if missing.is_empty() {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for line in missing {
        text.push_str(line);
        text.push('\n');
    }
    std::fs::write(&file, text).with_context(|| format!("writing {}", file.display()))
}

/// The id of a version, short, for messages.
pub fn short(id: Oid) -> String {
    id.to_string().chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn who(name: &str) -> Who {
        Who { name: name.into(), email: format!("{name}@example.com") }
    }

    fn scene(name: &str) -> (PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("xc-sync-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let bare = dir.join("stored.git");
        let mut options = git2::RepositoryInitOptions::new();
        options.bare(true).initial_head("main");
        Repository::init_opts(&bare, &options).unwrap();
        (dir.clone(), bare.to_string_lossy().replace('\\', "/"))
    }

    fn write(root: &Path, file: &str, text: &str) {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Two computers and one repository: storing, sending, fetching, and
    /// what happens when both changed something.
    #[test]
    fn two_computers_keep_one_project() {
        let (dir, url) = scene("two");
        let (a_dir, b_dir) = (dir.join("a"), dir.join("b"));
        let none = Login::default();
        let nothing = HashMap::new();

        // Computer A makes the project and stores it.
        fs::create_dir_all(&a_dir).unwrap();
        write(&a_dir, "circuit.project.json", "{\n  \"name\": \"Bot\"\n}\n");
        write(&a_dir, "workspaces/main.json", "{\n  \"name\": \"Main\",\n  \"nodes\": {}\n}\n");
        write(&a_dir, ".circuit/local.json", "{ \"hosting\": {} }");
        assert!(Sync::open(&a_dir).unwrap().is_none());
        let a = Sync::start(&a_dir).unwrap();
        assert_eq!(fs::read_to_string(a_dir.join(".gitignore")).unwrap(), ".circuit/\n");
        let state = a.state().unwrap();
        assert_eq!((state.branch.as_str(), state.remote.clone(), state.ahead), ("main", None, 0));
        assert!(state.changed.contains(&"workspaces/main.json".to_string()) && !state.changed.iter().any(|p| p.starts_with(".circuit")), "{:?}", state.changed);
        assert!(a.send(&none).unwrap_err().to_string().contains("isn't stored anywhere yet"));
        a.set_remote(&url).unwrap();
        assert!(a.store("The first version", &who("a")).unwrap());
        assert!(!a.store("Nothing new", &who("a")).unwrap(), "nothing changed: nothing recorded");
        assert_eq!(a.state().unwrap().ahead, 1);
        a.send(&none).unwrap();
        assert_eq!((a.state().unwrap().ahead, a.state().unwrap().changed.len()), (0, 0));

        // Computer B gets it: without A's own part.
        let b = Sync::get(&url, &b_dir, &none).unwrap();
        assert!(b_dir.join("workspaces/main.json").is_file() && !b_dir.join(".circuit").exists());

        // Only A changes something: B takes it as it is.
        write(&a_dir, "commands/ping.json", "{ \"name\": \"ping\" }\n");
        a.store("A ping command", &who("a")).unwrap();
        a.send(&none).unwrap();
        b.fetch(&none).unwrap();
        assert_eq!(b.state().unwrap().behind, 1);
        assert_eq!(b.merge(&nothing, &who("b")).unwrap(), Merged::Updated(vec!["commands/ping.json".into()]));
        assert!(b_dir.join("commands/ping.json").is_file());
        assert_eq!(b.merge(&nothing, &who("b")).unwrap(), Merged::UpToDate);

        // Both change different files: put together, nobody asked.
        write(&a_dir, "workspaces/a.json", "{ \"name\": \"A\" }\n");
        a.store("A's workspace", &who("a")).unwrap();
        a.send(&none).unwrap();
        write(&b_dir, "workspaces/b.json", "{ \"name\": \"B\" }\n");
        assert!(b.merge(&nothing, &who("b")).unwrap_err().to_string().contains("Store what changed here first"));
        b.store("B's workspace", &who("b")).unwrap();
        assert_eq!(b.send(&none).unwrap_err().to_string(), NEWER_THERE);
        b.fetch(&none).unwrap();
        assert_eq!(b.merge(&nothing, &who("b")).unwrap(), Merged::Updated(vec!["workspaces/a.json".into()]));
        assert!(b_dir.join("workspaces/a.json").is_file() && b_dir.join("workspaces/b.json").is_file());
        b.send(&none).unwrap();
        a.fetch(&none).unwrap();
        assert_eq!(a.merge(&nothing, &who("a")).unwrap(), Merged::Updated(vec!["workspaces/b.json".into()]));

        // Both change the same line: nothing changes until someone says whose.
        write(&a_dir, "workspaces/main.json", "{\n  \"name\": \"Main of A\",\n  \"nodes\": {}\n}\n");
        a.store("A renames", &who("a")).unwrap();
        a.send(&none).unwrap();
        write(&b_dir, "workspaces/main.json", "{\n  \"name\": \"Main of B\",\n  \"nodes\": {}\n}\n");
        write(&b_dir, "workspaces/b.json", "{ \"name\": \"B2\" }\n");
        b.store("B renames", &who("b")).unwrap();
        b.fetch(&none).unwrap();
        assert_eq!(b.merge(&nothing, &who("b")).unwrap(), Merged::Conflicts(vec!["workspaces/main.json".into()]));
        assert!(fs::read_to_string(b_dir.join("workspaces/main.json")).unwrap().contains("Main of B"), "untouched");
        assert!(b.state().unwrap().changed.is_empty(), "and not half merged");
        let Versions { mine, theirs } = b.versions("workspaces/main.json").unwrap();
        assert!(String::from_utf8(mine.unwrap()).unwrap().contains("Main of B") && String::from_utf8(theirs.unwrap()).unwrap().contains("Main of A"));
        let keep = HashMap::from([("workspaces/main.json".to_string(), Side::Theirs)]);
        assert_eq!(b.merge(&keep, &who("b")).unwrap(), Merged::Updated(vec!["workspaces/main.json".into()]));
        assert!(fs::read_to_string(b_dir.join("workspaces/main.json")).unwrap().contains("Main of A"));
        assert!(fs::read_to_string(b_dir.join("workspaces/b.json")).unwrap().contains("B2"), "B's other change stays");
        b.send(&none).unwrap();
        a.fetch(&none).unwrap();
        assert_eq!(a.merge(&nothing, &who("a")).unwrap(), Merged::Updated(vec!["workspaces/b.json".into()]));

        // One deletes a file the other changed; keeping mine keeps it deleted.
        fs::remove_file(a_dir.join("commands/ping.json")).unwrap();
        a.store("No ping", &who("a")).unwrap();
        a.send(&none).unwrap();
        write(&b_dir, "commands/ping.json", "{ \"name\": \"ping\", \"description\": \"Pong\" }\n");
        b.store("Ping says pong", &who("b")).unwrap();
        b.fetch(&none).unwrap();
        assert_eq!(b.merge(&nothing, &who("b")).unwrap(), Merged::Conflicts(vec!["commands/ping.json".into()]));
        let keep = HashMap::from([("commands/ping.json".to_string(), Side::Theirs)]);
        assert!(matches!(b.merge(&keep, &who("b")).unwrap(), Merged::Updated(_)));
        assert!(!b_dir.join("commands/ping.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Two changes that Git puts together line by line can give a file that
    /// is no JSON any more: that is asked about, not written.
    #[test]
    fn a_merged_file_that_no_longer_reads_is_asked_about() {
        let (dir, url) = scene("json");
        let (a_dir, b_dir) = (dir.join("a"), dir.join("b"));
        let none = Login::default();
        fs::create_dir_all(&a_dir).unwrap();
        // An empty object with room in it. A makes it an empty list, B puts
        // a member in: each is fine alone, and far enough apart for Git to
        // take both, which gives `[ "k": 1 ]`.
        let file = |open: &str, inside: &str, close: &str| format!("{{\n  \"list\": {open}\n\n\n\n\n{inside}\n\n\n\n\n  {close}\n}}\n");
        write(&a_dir, "workspaces/main.json", &file("{", "", "}"));
        let a = Sync::start(&a_dir).unwrap();
        a.set_remote(&url).unwrap();
        a.store("First", &who("a")).unwrap();
        a.send(&none).unwrap();
        let b = Sync::get(&url, &b_dir, &none).unwrap();
        write(&a_dir, "workspaces/main.json", &file("[", "", "]"));
        a.store("A", &who("a")).unwrap();
        a.send(&none).unwrap();
        write(&b_dir, "workspaces/main.json", &file("{", "    \"k\": 1", "}"));
        b.store("B", &who("b")).unwrap();
        b.fetch(&none).unwrap();
        assert_eq!(b.merge(&HashMap::new(), &who("b")).unwrap(), Merged::Conflicts(vec!["workspaces/main.json".into()]));
        let keep = HashMap::from([("workspaces/main.json".to_string(), Side::Mine)]);
        assert!(matches!(b.merge(&keep, &who("b")).unwrap(), Merged::Updated(_)));
        assert_eq!(fs::read_to_string(b_dir.join("workspaces/main.json")).unwrap(), file("{", "    \"k\": 1", "}"), "B's file as B had it");
        let _ = fs::remove_dir_all(&dir);
    }
}
