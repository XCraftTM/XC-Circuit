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
pub mod json;

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

/// A stored version, for the history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub id: String,
    /// The words that went with it.
    pub message: String,
    pub who: String,
    /// When, in seconds since 1970.
    pub when: i64,
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
            Ok(repo) => {
                // Also in a repository someone made by hand: this computer's
                // part (where the bot is uploaded to, its data) stays here.
                keep_ignored(root)?;
                Ok(Some(Sync { repo: keep_bytes(repo, root)? }))
            }
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
        Ok(Sync { repo: keep_bytes(sync.repo, root)? })
    }

    /// Makes an empty repository to store projects in, in the new folder
    /// `dir`: for a shared drive or a stick, where no GitHub is.
    pub fn start_bare(dir: &Path) -> Result<()> {
        let mut options = git2::RepositoryInitOptions::new();
        options.bare(true).initial_head("main");
        Repository::init_opts(dir, &options).with_context(|| format!("making a repository in {}", dir.display()))?;
        Ok(())
    }

    /// Gets a stored project into the new folder `root`.
    pub fn get(url: &str, root: &Path, login: &Login) -> Result<Sync> {
        let mut fetch = FetchOptions::new();
        fetch.remote_callbacks(callbacks(login));
        let repo = git2::build::RepoBuilder::new().fetch_options(fetch).clone(url, root).map_err(|e| explain(e, "getting the project"))?;
        let sync = Sync { repo: keep_bytes(repo, root)? };
        // Written again, now byte for byte.
        if sync.tip().is_some() {
            sync.checkout()?;
        }
        keep_ignored(root)?;
        Ok(sync)
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
        // Whatever of this computer's part got in before comes out again.
        for dir in IGNORED {
            index.remove_dir(Path::new(dir.trim_end_matches('/')), 0)?;
        }
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
        let (my_tree, their_tree) = (mine.tree()?, theirs.tree()?);
        // The project's files are put together by what they hold, not line
        // by line: blocks by their number, wires as a set, each setting on
        // its own. Lines know nothing of that, and can pair up the wrong
        // braces.
        let base_tree = self.repo.merge_base(mine.id(), theirs.id()).ok().and_then(|id| self.repo.find_commit(id).ok()).and_then(|c| c.tree().ok());
        let both: Vec<String> = {
            let theirs_changed = paths_of(&self.repo, base_tree.as_ref(), &their_tree)?;
            paths_of(&self.repo, base_tree.as_ref(), &my_tree)?.into_iter().filter(|p| p.ends_with(".json") && theirs_changed.contains(p)).collect()
        };
        for path in both {
            let read = |tree: &Tree| -> Option<(serde_json::Value, i32)> {
                let entry = tree.get_path(Path::new(&path)).ok()?;
                let blob = self.repo.find_blob(entry.id()).ok()?;
                Some((serde_json::from_slice(blob.content()).ok()?, entry.filemode()))
            };
            let (Some((a, mode)), Some((b, _))) = (read(&my_tree), read(&their_tree)) else { continue };
            let base = base_tree.as_ref().and_then(read).map(|(v, _)| v);
            match json::merge(base.as_ref(), &a, &b) {
                Some(merged) => {
                    let mut text = serde_json::to_string_pretty(&merged)?;
                    text.push('\n');
                    let id = self.repo.blob(text.as_bytes())?;
                    index.conflict_remove(Path::new(&path)).ok();
                    index.remove_path(Path::new(&path)).ok();
                    index.add(&entry(&path, id, mode as u32))?;
                }
                None => drop(unsure.insert(path)),
            }
        }
        for conflict in index.conflicts()? {
            let c = conflict?;
            if let Some(path) = [c.our, c.their, c.ancestor].into_iter().flatten().next().and_then(|e| String::from_utf8(e.path).ok()) {
                unsure.insert(path);
            }
        }
        // A JSON file put together from both that no longer reads is one too.
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
                index.add(&entry(path, e.id(), e.filemode() as u32))?;
            }
        }
        let tree = self.repo.find_tree(index.write_tree_to(&self.repo)?)?;
        let sig = signature(who)?;
        self.repo.commit(Some("HEAD"), &sig, &sig, "Put together with what was stored", &tree, &[&mine, &theirs])?;
        self.checkout()?;
        Ok(Merged::Updated(paths_of(&self.repo, Some(&before), &tree)?))
    }

    /// The newest stored versions, newest first.
    pub fn history(&self, limit: usize) -> Result<Vec<Version>> {
        let Some(tip) = self.tip() else { return Ok(Vec::new()) };
        let mut walk = self.repo.revwalk()?;
        walk.push(tip.id())?;
        walk.simplify_first_parent()?;
        let mut out = Vec::new();
        for id in walk.take(limit) {
            let commit = self.repo.find_commit(id?)?;
            out.push(Version { id: commit.id().to_string(), message: commit.summary().ok().flatten().unwrap_or_default().to_string(), who: commit.author().name().unwrap_or_default().to_string(), when: commit.time().seconds() });
        }
        Ok(out)
    }

    /// Makes the project what it was in the version `id`, as a new version
    /// on top: nothing in between is lost, and going back can be undone the
    /// same way. What changed here must be stored first. Gives the files
    /// that changed.
    pub fn go_back(&self, id: &str, who: &Who) -> Result<Vec<String>> {
        if !self.state()?.changed.is_empty() {
            bail!("Store what changed here first.");
        }
        let tip = self.tip().ok_or_else(|| anyhow!("Nothing is stored yet."))?;
        let then = self.repo.find_commit(Oid::from_str(id)?).map_err(|_| anyhow!("That version isn't in this project's history."))?;
        let (now, tree) = (tip.tree()?, then.tree()?);
        if now.id() == tree.id() {
            return Ok(Vec::new());
        }
        let sig = signature(who)?;
        let words = then.summary().ok().flatten().unwrap_or_default().to_string();
        self.repo.commit(Some("HEAD"), &sig, &sig, &format!("Back to: {words}"), &tree, &[&tip])?;
        self.checkout()?;
        paths_of(&self.repo, Some(&now), &tree)
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

/// Files go in and out byte for byte, whatever this computer's Git is set
/// to (Git for Windows turns line ends around by default): a project's
/// files are the same on every computer. The repository is opened again
/// after the setting changed, as an open one keeps the old setting.
fn keep_bytes(repo: Repository, root: &Path) -> Result<Repository> {
    let mut config = repo.config()?.open_level(git2::ConfigLevel::Local)?;
    if config.get_bool("core.autocrlf").ok() == Some(false) {
        return Ok(repo);
    }
    config.set_bool("core.autocrlf", false)?;
    config.set_bool("core.safecrlf", false)?;
    drop(config);
    drop(repo);
    Ok(Repository::open(root)?)
}

fn entry(path: &str, id: Oid, mode: u32) -> IndexEntry {
    IndexEntry { ctime: IndexTime::new(0, 0), mtime: IndexTime::new(0, 0), dev: 0, ino: 0, mode, uid: 0, gid: 0, file_size: 0, id, flags: 0, flags_extended: 0, path: path.as_bytes().to_vec() }
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

        // A repository made by hand, with this computer's part already in it:
        // it comes out with the next Store, and stays on the computer.
        let hand = dir.join("hand");
        write(&hand, "circuit.project.json", "{}
");
        write(&hand, ".circuit/local.json", "{}");
        let repo = Repository::init(&hand).unwrap();
        let mut index = repo.index().unwrap();
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = signature(&who("h")).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "By hand", &tree, &[]).unwrap();
        assert!(tree.get_path(Path::new(".circuit/local.json")).is_ok());
        let h = Sync::open(&hand).unwrap().unwrap();
        assert!(h.store("Tidy", &who("h")).unwrap());
        assert!(h.tip().unwrap().tree().unwrap().get_path(Path::new(".circuit/local.json")).is_err());
        assert!(hand.join(".circuit/local.json").is_file() && h.state().unwrap().changed.is_empty());

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

        // The history, newest first; going back makes a new version on top.
        let history = a.history(50).unwrap();
        assert_eq!(history.len(), a.history(3).unwrap().len().max(history.len()));
        assert!(history.iter().any(|v| v.message == "The first version" && v.who == "a") && history[0].when >= history.last().unwrap().when);
        let first = history.last().unwrap().clone();
        assert!(a_dir.join("workspaces/a.json").is_file());
        let changed = a.go_back(&first.id, &who("a")).unwrap();
        assert!(changed.contains(&"workspaces/a.json".to_string()) && !a_dir.join("workspaces/a.json").exists(), "{changed:?}");
        assert_eq!(a.history(1).unwrap()[0].message, "Back to: The first version");
        // And forward again, to where it was.
        let before = a.history(2).unwrap()[1].id.clone();
        a.go_back(&before, &who("a")).unwrap();
        assert!(a_dir.join("workspaces/a.json").is_file() && a.go_back(&before, &who("a")).unwrap().is_empty());
        assert!(a.go_back("0000000000000000000000000000000000000001", &who("a")).unwrap_err().to_string().contains("isn't in this project's history"));
        a.send(&none).unwrap();
        b.fetch(&none).unwrap();
        b.merge(&nothing, &who("b")).unwrap();

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

    /// Two people work in one workspace: their changes go together by what
    /// the file holds. Only the same setting changed two ways is asked about.
    #[test]
    fn a_workspace_is_put_together_by_its_blocks() {
        let (dir, url) = scene("blocks");
        let (a_dir, b_dir) = (dir.join("a"), dir.join("b"));
        let none = Login::default();
        let ws = |v: serde_json::Value| serde_json::to_string_pretty(&v).unwrap() + "\n";
        let json = serde_json::json!({ "name": "Main", "nodes": {
            "1": { "block": "e@1", "at": [0, 0] },
            "2": { "block": "log@1", "at": [300, 0], "values": { "message": "hi", "level": "info" } }
        }, "links": ["1.next > 2.in"] });
        fs::create_dir_all(&a_dir).unwrap();
        write(&a_dir, "workspaces/main.json", &ws(json.clone()));
        let a = Sync::start(&a_dir).unwrap();
        a.set_remote(&url).unwrap();
        a.store("First", &who("a")).unwrap();
        a.send(&none).unwrap();
        let b = Sync::get(&url, &b_dir, &none).unwrap();

        // A: changes the message, moves block 2, adds block 3 and its wire.
        let mut at_a = json.clone();
        at_a["nodes"]["2"]["values"]["message"] = "hello".into();
        at_a["nodes"]["2"]["at"] = serde_json::json!([300, 200]);
        at_a["nodes"]["3"] = serde_json::json!({ "block": "log@1", "at": [600, 0] });
        at_a["links"] = serde_json::json!(["1.next > 2.in", "2.next > 3.in"]);
        write(&a_dir, "workspaces/main.json", &ws(at_a));
        a.store("A", &who("a")).unwrap();
        a.send(&none).unwrap();
        // B: changes the level, moves block 2 elsewhere, adds block 4 and a
        // wire, renames the workspace, adds a note.
        let mut at_b = json.clone();
        at_b["name"] = "Welcome".into();
        at_b["nodes"]["2"]["values"]["level"] = "warn".into();
        at_b["nodes"]["2"]["at"] = serde_json::json!([900, 900]);
        at_b["nodes"]["4"] = serde_json::json!({ "block": "e@1", "at": [0, 400] });
        at_b["links"] = serde_json::json!(["1.next > 2.in", "4.next > 2.in"]);
        at_b["notes"] = serde_json::json!({ "n1": { "title": "Note", "rect": [0, 0, 10, 10] } });
        write(&b_dir, "workspaces/main.json", &ws(at_b));
        b.store("B", &who("b")).unwrap();
        b.fetch(&none).unwrap();
        assert_eq!(b.merge(&HashMap::new(), &who("b")).unwrap(), Merged::Updated(vec!["workspaces/main.json".into()]));
        let text = fs::read_to_string(b_dir.join("workspaces/main.json")).unwrap();
        let got: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(got, serde_json::json!({ "name": "Welcome", "nodes": {
            "1": { "block": "e@1", "at": [0, 0] },
            "2": { "block": "log@1", "at": [900, 900], "values": { "message": "hello", "level": "warn" } },
            "4": { "block": "e@1", "at": [0, 400] },
            "3": { "block": "log@1", "at": [600, 0] }
        }, "links": ["1.next > 2.in", "4.next > 2.in", "2.next > 3.in"], "notes": { "n1": { "title": "Note", "rect": [0, 0, 10, 10] } } }));
        assert_eq!(text, ws(got), "written as the app writes it");
        b.send(&none).unwrap();

        // The same setting changed two ways, and a block one deleted while
        // the other changed it: asked about.
        a.fetch(&none).unwrap();
        a.merge(&HashMap::new(), &who("a")).unwrap();
        let now: serde_json::Value = serde_json::from_str(&fs::read_to_string(a_dir.join("workspaces/main.json")).unwrap()).unwrap();
        let mut at_a = now.clone();
        at_a["nodes"]["2"]["values"]["message"] = "A says".into();
        write(&a_dir, "workspaces/main.json", &ws(at_a));
        a.store("A2", &who("a")).unwrap();
        a.send(&none).unwrap();
        let mut at_b = now.clone();
        at_b["nodes"]["2"]["values"]["message"] = "B says".into();
        write(&b_dir, "workspaces/main.json", &ws(at_b));
        b.store("B2", &who("b")).unwrap();
        b.fetch(&none).unwrap();
        assert_eq!(b.merge(&HashMap::new(), &who("b")).unwrap(), Merged::Conflicts(vec!["workspaces/main.json".into()]));
        let _ = fs::remove_dir_all(&dir);
    }
}
