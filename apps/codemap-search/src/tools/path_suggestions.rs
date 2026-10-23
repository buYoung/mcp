//! Similar existing paths for a `read`/`grep`/`find` path that does not exist. A guessed or
//! mistyped path then points at the intended file from the error itself, without another lookup.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;
use std::time::{Duration, Instant};

/// Paths named in one error message.
const MAX_SUGGESTIONS: usize = 3;
/// Total walk time per error, so the error path stays cheap in large repositories.
const WALK_BUDGET: Duration = Duration::from_millis(300);

/// Exact name, then same kind (same extension, or a folder for an extensionless name), then name
/// similarity, then shared folder names, then shorter path.
type Rank = (bool, bool, u8, usize, Reverse<usize>);

/// `". Similar existing paths: a, b"` for a missing path inside the workspace, or empty when it
/// lies outside the workspace or nothing there resembles it.
pub(crate) fn note(missing: &Path) -> String {
    let paths = similar_paths(missing);
    if paths.is_empty() {
        String::new()
    } else {
        format!(". Similar existing paths: {}", paths.join(", "))
    }
}

fn similar_paths(missing: &Path) -> Vec<String> {
    let Some(target) = missing
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(Name::new)
    else {
        return Vec::new();
    };
    let Ok(cwd) = crate::workspace::current_dir() else {
        return Vec::new();
    };
    let cwd = cwd.canonicalize().unwrap_or(cwd);
    let Ok(relative) = missing.strip_prefix(&cwd) else {
        return Vec::new();
    };
    let Some(ancestor) = missing.ancestors().skip(1).find(|path| path.is_dir()) else {
        return Vec::new();
    };
    let wanted_dirs: HashSet<&OsStr> = relative
        .parent()
        .map(|parent| parent.iter().collect())
        .unwrap_or_default();
    let started = Instant::now();
    let mut ranked = Vec::new();
    // The guessed prefix usually holds the intended file, so it is walked first; the rest of
    // the workspace follows within the budget, where an exact name elsewhere still ranks first.
    collect(ancestor, &target, &wanted_dirs, &cwd, started, &mut ranked);
    if ancestor != cwd {
        collect(&cwd, &target, &wanted_dirs, &cwd, started, &mut ranked);
    }
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    ranked.dedup_by(|left, right| left.1 == right.1);
    ranked
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, path)| path)
        .collect()
}

fn collect(
    root: &Path,
    target: &Name,
    wanted_dirs: &HashSet<&OsStr>,
    cwd: &Path,
    started: Instant,
    ranked: &mut Vec<(Rank, String)>,
) {
    for entry in crate::workspace::build_walker(root, false)
        .build()
        .flatten()
    {
        if started.elapsed() > WALK_BUDGET {
            return;
        }
        if entry.depth() == 0 {
            continue;
        }
        let path = entry.path();
        let Some(level) = path
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(|name| target.similarity(name))
        else {
            continue;
        };
        let relative = path.strip_prefix(cwd).unwrap_or(path);
        let shared_dirs = relative.parent().map_or(0, |parent| {
            parent
                .iter()
                .filter(|part| wanted_dirs.contains(part))
                .count()
        });
        let is_same_kind = match &target.extension {
            Some(extension) => path
                .extension()
                .and_then(OsStr::to_str)
                .is_some_and(|other| other.eq_ignore_ascii_case(extension)),
            None => entry.file_type().is_some_and(|kind| kind.is_dir()),
        };
        let display = relative.display().to_string();
        ranked.push((
            (
                level == 3,
                is_same_kind,
                level,
                shared_dirs,
                Reverse(display.len()),
            ),
            display,
        ));
    }
}

struct Name {
    lower: String,
    /// Lowercase letters and digits of the name without its last extension.
    stem: String,
    /// Lowercase words of that stem, split at separators and lower-to-upper case changes.
    words: Vec<String>,
    /// Lowercase extension; names without one are usually folders.
    extension: Option<String>,
}

impl Name {
    fn new(name: &str) -> Option<Self> {
        let path = Path::new(name);
        let raw_stem = path.file_stem()?.to_str()?;
        let stem: String = raw_stem
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        (stem.chars().count() >= 3).then(|| Self {
            lower: name.to_lowercase(),
            stem,
            words: words(raw_stem),
            extension: path
                .extension()
                .and_then(OsStr::to_str)
                .map(str::to_lowercase),
        })
    }

    /// 3 for the same name, 2 when only case, separators or the extension differ, 1 when the
    /// candidate extends the name, covers most of it, holds all its words or is a typo apart;
    /// `None` otherwise.
    fn similarity(&self, candidate: &str) -> Option<u8> {
        let other = Name::new(candidate)?;
        if other.lower == self.lower {
            return Some(3);
        }
        if other.stem == self.stem {
            return Some(2);
        }
        let extends = self.stem.len() >= 4 && other.stem.starts_with(&self.stem);
        // A shorter candidate must cover most of the name, so `payment.md` never stands in for
        // `payment-method-data-context.tsx`.
        let covers = other.stem.len() >= 4
            && self.stem.starts_with(&other.stem)
            && other.stem.len() * 5 >= self.stem.len() * 3;
        let has_all_words =
            self.words.len() >= 2 && self.words.iter().all(|word| other.words.contains(word));
        let typo_limit = match self.stem.chars().count() {
            0..=3 => 0,
            4..=7 => 1,
            _ => 2,
        };
        let is_typo = typo_limit > 0 && is_within_edits(&self.stem, &other.stem, typo_limit);
        (extends || covers || has_all_words || is_typo).then_some(1)
    }
}

fn words(stem: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut is_after_lowercase = false;
    for c in stem.chars() {
        if !c.is_alphanumeric() {
            words.extend((!current.is_empty()).then(|| std::mem::take(&mut current)));
            is_after_lowercase = false;
            continue;
        }
        if c.is_uppercase() && is_after_lowercase {
            words.push(std::mem::take(&mut current));
        }
        is_after_lowercase = c.is_lowercase() || c.is_ascii_digit();
        current.extend(c.to_lowercase());
    }
    words.extend((!current.is_empty()).then_some(current));
    words.retain(|word| word.chars().count() >= 2);
    words
}

/// Whether `left` becomes `right` with at most `limit` single-character edits.
fn is_within_edits(left: &str, right: &str, limit: usize) -> bool {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.len().abs_diff(right.len()) > limit {
        return false;
    }
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (row, left_char) in left.iter().enumerate() {
        let mut current = vec![row + 1; right.len() + 1];
        for (column, right_char) in right.iter().enumerate() {
            current[column + 1] = (previous[column] + usize::from(left_char != right_char))
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        if current.iter().min().is_some_and(|&least| least > limit) {
            return false;
        }
        previous = current;
    }
    previous[right.len()] <= limit
}
