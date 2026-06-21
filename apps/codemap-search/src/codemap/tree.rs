use super::summary::DirectorySummary;

type ChildMap<'a> = std::collections::HashMap<&'a str, Vec<&'a DirectorySummary>>;

/// Max child groups rendered inline on one directory row. The row count cap alone is not
/// enough for very wide repos because one anchor can otherwise inline hundreds of siblings.
const INLINE_CHILD_LIMIT: usize = 12;

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn directory_label(path: &str, file_count: usize, symbol_count: usize) -> String {
    format!(
        "{} ({} files, {} symbols)",
        basename(path),
        file_count,
        symbol_count
    )
}

/// A directory is a leaf when it has no subdirectories.
fn dir_is_leaf(children: &ChildMap, path: &str) -> bool {
    children.get(path).is_none_or(|kids| kids.is_empty())
}

/// "Deep" = has at least one non-leaf subdirectory (its subtree is ≥2 levels).
fn dir_is_deep(children: &ChildMap, path: &str) -> bool {
    children
        .get(path)
        .is_some_and(|kids| kids.iter().any(|c| !dir_is_leaf(children, &c.path)))
}

/// True when at least one immediate subdirectory is itself a leaf.
fn dir_has_leaf_child(children: &ChildMap, path: &str) -> bool {
    children
        .get(path)
        .is_some_and(|kids| kids.iter().any(|c| dir_is_leaf(children, &c.path)))
}

/// How a child renders *inline* on its parent's line:
/// - leaf → `name (counts)`
/// - terminal-group (all of its children are leaves) → `name (counts): leafA (counts), …`
/// - deep (has a non-leaf child) → `name (counts)` (bare; its subtree continues on its
///   own anchor line(s))
fn render_inline_child(children: &ChildMap, child: &DirectorySummary) -> String {
    let head = directory_label(&child.path, child.file_count, child.symbol_count);
    if dir_is_leaf(children, &child.path) || dir_is_deep(children, &child.path) {
        head
    } else {
        // terminal-group: inline its leaf children one level deep, wrapped in braces so
        // the nested group is unambiguous against the parent's own sibling list
        // (`auth: {a, b}, common` — `common` is clearly the parent's child, not auth's).
        let leaf_children = &children[child.path.as_str()];
        let mut leaves = leaf_children
            .iter()
            .take(INLINE_CHILD_LIMIT)
            .map(|g| directory_label(&g.path, g.file_count, g.symbol_count))
            .collect::<Vec<_>>();
        if leaf_children.len() > INLINE_CHILD_LIMIT {
            leaves.push(format!(
                "+{} more; use `overview {}`",
                leaf_children.len() - INLINE_CHILD_LIMIT,
                child.path
            ));
        }
        let leaves = leaves.join(", ");
        format!("{}: {{{}}}", head, leaves)
    }
}

/// Render a directory tree with Rust-`use`-style inlining. Each *anchor* directory
/// occupies one line that inlines all its immediate children (see [`render_inline_child`]);
/// the repeated parent-path prefix — the dominant redundancy on a deep tree — is written
/// once. Which directories get their own line:
/// - every immediate child of `scope` is an anchor (`scope` is `""` for the repo root, or
///   a folder path like `src/modules` for a folder view);
/// - a `deep` child of an anchor that *has a leaf child of its own* becomes an anchor
///   (it has direct files/leaf-dirs worth breaking out, e.g. `src/common`);
/// - a `deep` child with *no* leaf child (a pure junction, e.g. `src/common/modules`)
///   gets no line — it is only bare-mentioned on its parent, and its subdirectories are
///   promoted to anchors instead.
///
/// The full repo `directories` slice is passed regardless of scope; only directories
/// reachable from `scope` are seeded, so the rest never render. Output is bounded by
/// `limit` emitted anchor lines and [`INLINE_CHILD_LIMIT`] inline children per anchor so a
/// pathologically wide tree can't blow the budget.
pub(crate) fn write_directory_tree(
    f: &mut std::fmt::Formatter<'_>,
    directories: &[DirectorySummary],
    scope: &str,
    limit: usize,
) -> std::fmt::Result {
    use std::collections::{BTreeSet, HashMap};

    // Immediate children keyed by parent path (top-level directories sit under "").
    // `directories` is path-sorted, so each child list stays alphabetical.
    let mut children: ChildMap = HashMap::new();
    for dir in directories {
        let parent = match dir.path.rfind('/') {
            Some(slash) => &dir.path[..slash],
            None => "",
        };
        children.entry(parent).or_default().push(dir);
    }

    // Decide the anchor set (directories that get their own line). A small worklist
    // walks down from `scope`'s immediate children: an anchor exposes its deep children,
    // and a deep child either becomes an anchor (it has a leaf child) or is skipped so its
    // own children are considered in turn.
    enum Task<'a> {
        Anchor(&'a str),
        Consider(&'a str),
    }
    let mut anchors: BTreeSet<&str> = BTreeSet::new();
    let mut stack: Vec<Task> = Vec::new();
    if let Some(tops) = children.get(scope) {
        for dir in tops {
            stack.push(Task::Anchor(dir.path.as_str()));
        }
    }
    while let Some(task) = stack.pop() {
        match task {
            Task::Anchor(path) => {
                if !anchors.insert(path) {
                    continue;
                }
                if let Some(kids) = children.get(path) {
                    for child in kids {
                        if dir_is_deep(&children, &child.path) {
                            stack.push(Task::Consider(child.path.as_str()));
                        }
                    }
                }
            }
            Task::Consider(path) => {
                if dir_has_leaf_child(&children, path) {
                    stack.push(Task::Anchor(path));
                } else if let Some(kids) = children.get(path) {
                    for child in kids {
                        stack.push(Task::Consider(child.path.as_str()));
                    }
                }
            }
        }
    }

    let by_path: HashMap<&str, &DirectorySummary> =
        directories.iter().map(|d| (d.path.as_str(), d)).collect();
    let total = anchors.len();
    for path in anchors.iter().take(limit) {
        let dir = by_path[path];
        match children.get(path) {
            Some(kids) if !kids.is_empty() => {
                let mut inlined = kids
                    .iter()
                    .take(INLINE_CHILD_LIMIT)
                    .map(|c| render_inline_child(&children, c))
                    .collect::<Vec<_>>();
                if kids.len() > INLINE_CHILD_LIMIT {
                    inlined.push(format!(
                        "+{} more; use `overview {}` or `find`",
                        kids.len() - INLINE_CHILD_LIMIT,
                        dir.path
                    ));
                }
                let inlined = inlined.join(", ");
                writeln!(
                    f,
                    "- {} ({} files, {} symbols): {}",
                    dir.path, dir.file_count, dir.symbol_count, inlined
                )?;
            }
            _ => writeln!(
                f,
                "- {} ({} files, {} symbols)",
                dir.path, dir.file_count, dir.symbol_count
            )?,
        }
    }
    if total > limit {
        writeln!(
            f,
            "_… {} more directory groups not shown; narrow with `overview <dir>`._",
            total - limit
        )?;
    }
    Ok(())
}
