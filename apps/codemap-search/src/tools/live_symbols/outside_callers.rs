//! Caller trailer for `read view=source`: who calls the functions in the window from outside
//! it. Source rows above the trailer stay byte-for-byte unchanged.

use super::structure::callable;
use super::LiveOutput;
use crate::callers::{AnnotationRequest, CallerSite};
use crate::index::EngineSupervisor;

/// Functions overlapping one window whose callers are looked up.
const FUNCTION_LOOKUP_LIMIT: usize = 16;
/// Callers listed per function before the rest are counted.
const CALLERS_PER_FUNCTION: usize = 3;
/// Bytes for the whole trailer, including its header.
const TRAILER_BYTE_CAP: usize = 1536;
pub(crate) const HEADER: &str = "[Callers from outside this window (static analysis, approximate)]";

/// The trailer for a read window, within `byte_cap` bytes and never above [`TRAILER_BYTE_CAP`].
/// Nothing is looked up when the space cannot hold a header and one function line.
pub(super) fn trailer(
    engine: &EngineSupervisor,
    output: &LiveOutput,
    byte_cap: usize,
) -> Option<String> {
    let byte_cap = byte_cap.min(TRAILER_BYTE_CAP);
    // Room for the closing omission line is kept whenever a function line is added.
    let reserve = omission_line(FUNCTION_LOOKUP_LIMIT).len();
    if !output.should_list_outside_callers || byte_cap < HEADER.len() + reserve + 64 {
        return None;
    }
    let [anchor] = output.anchors.as_slice() else {
        return None;
    };
    let (start, end) = anchor.start_line.zip(anchor.end_line)?;
    if engine.is_warming() || engine.is_dead() || engine.last_error().is_some() {
        return None;
    }
    let root = std::env::current_dir().ok()?;
    let path = anchor.file_path.as_str();
    let filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
    if filter.is_file_excluded(path)
        || crate::workspace::source_encoding_exclusion(&root.join(path)).is_some()
    {
        return None;
    }
    let snapshot = engine.published_snapshot();
    let source_files = snapshot.codemap();
    let indexed = source_files.iter().find(|file| file.file_path == path);
    // Stale or unindexed files would attribute callers to the wrong lines.
    if super::diagnostics::unavailable(indexed, anchor, &root, snapshot.flows().digest(path))
        .is_some()
    {
        return None;
    }
    let filtered = filter.filter_snapshot(&source_files, |file| file.file_path == path);
    let file = filtered.iter().find(|file| file.file_path == path)?;
    if file.macro_expansion().is_some() {
        return None;
    }
    let functions: Vec<_> = file
        .symbols
        .iter()
        .filter(|symbol| {
            callable(symbol)
                && symbol.range.start_line <= end
                && start <= symbol.range.end_line_inclusive()
        })
        .take(FUNCTION_LOOKUP_LIMIT)
        .collect();
    if functions.is_empty() {
        return None;
    }
    let requests: Vec<_> = functions
        .iter()
        .map(|symbol| AnnotationRequest {
            file_path: path,
            symbols: std::slice::from_ref(*symbol),
            is_fallback: false,
        })
        .collect();
    let resolver = crate::callers::resolution::SourceResolver::new(&source_files, &root);
    let annotations = crate::callers::annotate_live_callers(
        &requests,
        &source_files,
        &super::render::caller_config(),
        &root,
        &resolver,
    )?;
    let is_outside = |site: &&CallerSite| site.path != path || site.line < start || end < site.line;
    let mut text = String::from(HEADER);
    let mut omitted_functions = 0;
    for symbol in functions {
        let Some((callers, unconfirmed)) =
            annotations.caller_sites(path, symbol.range.start_line, symbol.range.start_col)
        else {
            continue;
        };
        let mut sites: Vec<_> = callers
            .iter()
            .filter(is_outside)
            .map(|site| (site, false))
            .chain(
                unconfirmed
                    .iter()
                    .filter(is_outside)
                    .map(|site| (site, true)),
            )
            .collect();
        if sites.is_empty() {
            continue;
        }
        // Other files first; the stable sort keeps discovery order within each group.
        sites.sort_by_key(|(site, _)| site.path == path);
        let groups = group_by_caller(&sites);
        let shown: Vec<_> = groups
            .iter()
            .take(CALLERS_PER_FUNCTION)
            .map(|group| label(group, path))
            .collect();
        let name = symbol.name.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut line = format!(
            "\n- {name} L{}: {}",
            symbol.range.start_line,
            shown.join(", ")
        );
        if groups.len() > CALLERS_PER_FUNCTION {
            line.push_str(&format!(
                ", +{} more (view=relations)",
                groups.len() - CALLERS_PER_FUNCTION
            ));
        }
        if omitted_functions > 0 || text.len() + line.len() + reserve > byte_cap {
            omitted_functions += 1;
            continue;
        }
        text.push_str(&line);
    }
    if text.len() == HEADER.len() {
        return None;
    }
    if omitted_functions > 0 {
        text.push_str(&omission_line(omitted_functions));
    }
    Some(text)
}

fn omission_line(count: usize) -> String {
    format!("\n- {count} more function(s) with outside callers (view=relations)")
}

/// Call sites of one caller in one file, in discovery order.
struct CallerGroup<'a> {
    site: &'a CallerSite,
    is_unconfirmed: bool,
    lines: Vec<usize>,
}

fn group_by_caller<'a>(sites: &[(&'a CallerSite, bool)]) -> Vec<CallerGroup<'a>> {
    let mut groups: Vec<CallerGroup<'a>> = Vec::new();
    for &(site, is_unconfirmed) in sites {
        match groups.iter_mut().find(|group| {
            group.is_unconfirmed == is_unconfirmed
                && group.site.caller == site.caller
                && group.site.path == site.path
        }) {
            Some(group) => group.lines.push(site.line),
            None => groups.push(CallerGroup {
                site,
                is_unconfirmed,
                lines: vec![site.line],
            }),
        }
    }
    groups
}

fn label(group: &CallerGroup<'_>, path: &str) -> String {
    let lines = group
        .lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let location = if group.site.path == path {
        format!("L{}", lines.join(", L"))
    } else {
        format!("{}:{}", group.site.path, lines.join(", "))
    };
    let caller = group.site.caller.as_deref().unwrap_or("top-level");
    let mark = if group.is_unconfirmed {
        "unconfirmed "
    } else {
        ""
    };
    format!("{mark}{caller} ({location})")
}
