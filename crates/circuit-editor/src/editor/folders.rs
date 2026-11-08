//! Folders in folders, for the side bar's workspaces and slash commands.
//!
//! A folder is a path of names: `Tickets/Archive` is the Archive folder in
//! Tickets. Projects keep the folders as a list of paths, in the order they
//! were dragged into (siblings are shown in that order), and each workspace or
//! command names the folder it is in. Projects from before folders could nest
//! have only one-name paths, so they read the same.

/// What separates the names in a folder's path.
pub const SEP: char = '/';

/// The folder's own name: the last of its path.
pub fn name(path: &str) -> &str {
    path.rsplit(SEP).next().unwrap_or(path)
}

/// The folder it is in, if any.
pub fn parent(path: &str) -> Option<&str> {
    path.rsplit_once(SEP).map(|(p, _)| p)
}

/// How deep it is: 0 for a folder at the top.
pub fn depth(path: &str) -> usize {
    path.matches(SEP).count()
}

/// The path of a folder called `name` in `parent` (or at the top). A `/` in
/// the name would make it two folders, so it becomes a `-`.
pub fn child(parent: Option<&str>, name: &str) -> String {
    let name = name.trim().replace(SEP, "-");
    match parent {
        Some(p) => format!("{p}{SEP}{name}"),
        None => name,
    }
}

/// Whether `path` is `folder` or a folder somewhere inside it.
pub fn within(path: &str, folder: &str) -> bool {
    path == folder || path.strip_prefix(folder).is_some_and(|rest| rest.starts_with(SEP))
}

/// The path for display: its names joined with ›.
pub fn shown(path: &str) -> String {
    path.split(SEP).collect::<Vec<_>>().join(" › ")
}

/// Every folder, in the order the tree shows them (each followed by the ones
/// in it): the `listed` ones, the ones only `used` by workspaces or commands,
/// and every folder those are in. Siblings keep their order in `listed`;
/// unlisted ones come after them.
pub fn ordered<'a>(listed: &[String], used: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    let add = |path: &str, all: &mut Vec<String>| {
        // Its parents too, so a folder named only through one inside it still shows.
        for end in path.match_indices(SEP).map(|(i, _)| i).chain(std::iter::once(path.len())) {
            let p = &path[..end];
            if !p.is_empty() && !all.iter().any(|a| a == p) {
                all.push(p.to_string());
            }
        }
    };
    for p in listed {
        add(p, &mut all);
    }
    for p in used {
        add(p, &mut all);
    }
    let rank = |p: &str| listed.iter().position(|l| l == p).unwrap_or(usize::MAX);
    let mut out = Vec::with_capacity(all.len());
    fn walk(all: &[String], parent: Option<&str>, rank: &dyn Fn(&str) -> usize, out: &mut Vec<String>) {
        let mut kids: Vec<(usize, usize, &String)> = all.iter().enumerate().filter(|(_, p)| self::parent(p) == parent).map(|(i, p)| (rank(p), i, p)).collect();
        kids.sort();
        for (_, _, k) in kids {
            out.push(k.clone());
            walk(all, Some(k), rank, out);
        }
    }
    walk(&all, None, &rank, &mut out);
    out
}

/// The folders right inside `parent` (or at the top), in `ordered`'s order.
pub fn children<'a>(ordered: &'a [String], parent: Option<&str>) -> impl Iterator<Item = &'a String> {
    ordered.iter().filter(move |p| self::parent(p) == parent)
}

/// `path` with folder `old` (and so everything in it) now at `new`; None
/// when it isn't in `old`.
pub fn moved(path: &str, old: &str, new: &str) -> Option<String> {
    within(path, old).then(|| format!("{new}{}", &path[old.len()..]))
}

/// Moves folder `old` with everything in it to `new` (a rename, or a move
/// into another folder): in the list and for every workspace or command in
/// it (`items`, each one's folder). A folder already at `new` takes in what
/// comes. Nothing happens when `new` is inside `old`.
pub fn move_folder<'a>(list: &mut Vec<String>, items: impl IntoIterator<Item = &'a mut Option<String>>, old: &str, new: &str) -> bool {
    if old == new || within(new, old) || new.is_empty() {
        return false;
    }
    for p in list.iter_mut() {
        if let Some(m) = moved(p, old, new) {
            *p = m;
        }
    }
    let mut seen = std::collections::HashSet::new();
    list.retain(|p| seen.insert(p.clone()));
    if !list.iter().any(|p| p == new) {
        list.push(new.to_string());
    }
    for f in items.into_iter().flatten() {
        if let Some(m) = moved(f, old, new) {
            *f = m;
        }
    }
    true
}

/// Deletes `folder`: what's in it (workspaces or commands, and folders) goes
/// up into the folder it was in, or to the top.
pub fn delete_folder<'a>(list: &mut Vec<String>, items: impl IntoIterator<Item = &'a mut Option<String>>, folder: &str) {
    let up = parent(folder).map(str::to_string);
    let lift = |p: &str| -> Option<String> {
        let rest = p.strip_prefix(folder)?.strip_prefix(SEP)?;
        Some(match &up {
            Some(u) => format!("{u}{SEP}{rest}"),
            None => rest.to_string(),
        })
    };
    let mut kept = Vec::with_capacity(list.len());
    for p in list.drain(..) {
        if p == folder {
            continue;
        }
        let p = lift(&p).unwrap_or(p);
        if !kept.contains(&p) {
            kept.push(p);
        }
    }
    *list = kept;
    for f in items {
        match f.as_deref() {
            Some(p) if p == folder => *f = up.clone(),
            Some(p) => {
                if let Some(l) = lift(p) {
                    *f = Some(l);
                }
            }
            None => {}
        }
    }
}

/// Puts folder `path` right before (or after) folder `next_to` in the list:
/// with it, in the folder `next_to` is in. Returns where it is now.
pub fn place_folder<'a>(list: &mut Vec<String>, items: impl IntoIterator<Item = &'a mut Option<String>>, path: &str, next_to: &str, before: bool) -> Option<String> {
    if within(next_to, path) {
        return None;
    }
    let new = landing(list, path, parent(next_to));
    if new != path && !move_folder(list, items, path, &new) {
        return None;
    }
    list.retain(|p| *p != new);
    let at = list.iter().position(|p| p == next_to).map_or(list.len(), |i| if before { i } else { i + 1 });
    list.insert(at, new.clone());
    Some(new)
}

/// Puts folder `path` into folder `into` (or at the top), after the ones
/// already there. Returns where it is now.
pub fn nest_folder<'a>(list: &mut Vec<String>, items: impl IntoIterator<Item = &'a mut Option<String>>, path: &str, into: Option<&str>) -> Option<String> {
    if into.is_some_and(|i| within(i, path)) || parent(path) == into {
        return None;
    }
    let new = landing(list, path, into);
    move_folder(list, items, path, &new).then(|| {
        // Last among its new siblings.
        list.retain(|p| *p != new);
        list.push(new.clone());
        new
    })
}

/// Where folder `path` lands in `into`: under its own name, or (when
/// another folder there has it) a free one, so the two don't merge.
fn landing(list: &[String], path: &str, into: Option<&str>) -> String {
    let new = child(into, name(path));
    if new == path || !list.contains(&new) { new } else { child(into, &free_name(list, into, name(path))) }
}

/// A free name for a new folder in `parent`: `base`, else `base 2` …
pub fn free_name(all: &[String], parent: Option<&str>, base: &str) -> String {
    (1..).map(|i| if i == 1 { base.to_string() } else { format!("{base} {i}") }).find(|n| !all.contains(&child(parent, n))).expect("unbounded")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn paths() {
        assert_eq!(name("Tickets/Archive"), "Archive");
        assert_eq!(parent("Tickets/Archive"), Some("Tickets"));
        assert_eq!(parent("Tickets"), None);
        assert_eq!(depth("A/B/C"), 2);
        assert_eq!(child(Some("A"), " B/C "), "A/B-C");
        assert!(within("A/B", "A") && within("A", "A") && !within("AB", "A"));
        assert_eq!(shown("A/B"), "A › B");
    }

    #[test]
    fn the_tree_order_puts_each_folder_before_whats_in_it() {
        let listed = s(&["Events", "Tickets", "Tickets/Archive", "Events/Old"]);
        assert_eq!(ordered(&listed, ["Fun/Games"]), ["Events", "Events/Old", "Tickets", "Tickets/Archive", "Fun", "Fun/Games"]);
        // Old projects: one-name folders, as listed.
        assert_eq!(ordered(&s(&["B", "A"]), ["C"]), ["B", "A", "C"]);
    }

    #[test]
    fn folders_move_rename_and_delete_with_whats_in_them() {
        let mut list = s(&["A", "A/X", "B"]);
        let mut items = vec![Some("A".to_string()), Some("A/X".to_string()), None];
        // A into B: its folder and workspaces go along.
        assert_eq!(nest_folder(&mut list, items.iter_mut(), "A", Some("B")), Some("B/A".into()));
        assert_eq!(ordered(&list, []), ["B", "B/A", "B/A/X"]);
        assert_eq!(items, [Some("B/A".into()), Some("B/A/X".into()), None]);
        // Not into itself.
        assert_eq!(nest_folder(&mut list, items.iter_mut(), "B", Some("B/A")), None);
        // Renamed.
        assert!(move_folder(&mut list, items.iter_mut(), "B/A", "B/Old"));
        assert_eq!(items[1].as_deref(), Some("B/Old/X"));
        // Next to a top folder: back at the top, before B.
        assert_eq!(place_folder(&mut list, items.iter_mut(), "B/Old/X", "B", true), Some("X".into()));
        assert_eq!(ordered(&list, []), ["X", "B", "B/Old"]);
        assert_eq!(items[1].as_deref(), Some("X"));
        // Deleting B: what's in it goes up.
        delete_folder(&mut list, items.iter_mut(), "B");
        assert_eq!(ordered(&list, []), ["X", "Old"]);
        assert_eq!(items, [Some("Old".into()), Some("X".into()), None]);
        assert_eq!(free_name(&list, None, "Old"), "Old 2");
    }
}
