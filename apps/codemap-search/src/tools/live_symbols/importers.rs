//! Importer trailer for `read view=source`: other files that import the window's exported
//! objects, classes and constants, and where they use them. Callers reach functions by name,
//! but such a value reaches its users through imports, which the index records. Source rows
//! above the trailer stay unchanged.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use super::shared_keys::enclosing_label;
use super::structure::{callable, container};
use super::LiveOutput;
use crate::callers::resolution::SourceResolver;
use crate::index::EngineSupervisor;
use crate::parser::{ExtractedFile, ExtractedSymbol, ImportEntry, ImportKind, NavigationFile};

/// Declarations taken from one window, in order of appearance.
const DECLARATION_LIMIT: usize = 16;
/// Importing files listed per declaration before the rest are counted.
const FILES_PER_DECLARATION: usize = 6;
/// Places (an enclosing function, or a top-level line) listed per importing file.
const PLACES_PER_FILE: usize = 3;
/// Bytes for the whole trailer, including its header.
const TRAILER_BYTE_CAP: usize = 1536;
/// Time for scanning the index and resolving the matching imports. Importers found by then are
/// listed with a note that the lookup stopped.
const TRAILER_TIME_BUDGET: Duration = Duration::from_millis(100);
const PARTIAL_LINE: &str =
    "\n- import lookup stopped at its time limit; more importing files may exist (grep the name)";
pub(crate) const HEADER: &str =
    "[Other files importing this window's declarations (static analysis, approximate)]";

/// One declaration of the window and the files importing it, with the lines using it there.
struct Declaration<'a> {
    symbol: &'a ExtractedSymbol,
    importers: Vec<(usize, Vec<usize>)>,
}

/// The trailer for a read window, within `byte_cap` bytes and never above [`TRAILER_BYTE_CAP`].
pub(super) fn trailer(
    engine: &EngineSupervisor,
    output: &LiveOutput,
    byte_cap: usize,
) -> Option<String> {
    let started = Instant::now();
    let byte_cap = byte_cap.min(TRAILER_BYTE_CAP);
    if !output.should_list_outside_callers || byte_cap < HEADER.len() + PARTIAL_LINE.len() + 64 {
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
    if filter.is_file_excluded(path) {
        return None;
    }
    let snapshot = engine.published_snapshot();
    let files = snapshot.codemap();
    let file = files.iter().find(|file| file.file_path == path)?;
    // Stale or unindexed files would point at the wrong declarations.
    if super::diagnostics::unavailable(Some(file), anchor, &root, snapshot.flows().digest(path))
        .is_some()
    {
        return None;
    }
    let mut declarations = window_declarations(file, start, end);
    if declarations.is_empty() {
        return None;
    }
    let by_name: HashMap<&str, usize> = declarations
        .iter()
        .enumerate()
        .map(|(index, declaration)| (declaration.symbol.name.as_str(), index))
        .collect();
    let deadline = started + TRAILER_TIME_BUDGET;
    let mut is_partial = false;
    // Imports naming a declaration, found by name first; only those are resolved.
    let mut candidates = Vec::new();
    for (index, other) in files.iter().enumerate() {
        if index % 64 == 0 && Instant::now() >= deadline {
            is_partial = true;
            break;
        }
        if other.file_path == path || filter.is_file_excluded(&other.file_path) {
            continue;
        }
        let Some(navigation) = &other.navigation else {
            continue;
        };
        for import in &navigation.imports {
            for (name, lines) in imported_uses(navigation, import, &by_name) {
                candidates.push((index, import, name, lines));
            }
        }
    }
    if !candidates.is_empty() {
        let resolver = SourceResolver::new(&files, &root);
        for (index, import, name, lines) in candidates {
            if Instant::now() >= deadline {
                is_partial = true;
                break;
            }
            if imports_from(&resolver, &files[index], import, name, path) {
                let declaration = &mut declarations[by_name[name]];
                if !declaration.importers.iter().any(|(file, _)| *file == index) {
                    declaration.importers.push((index, lines));
                }
            }
        }
    }
    trailer_text(&files, &declarations, is_partial, byte_cap)
}

/// Exported top-level values overlapping lines `start..=end`. Functions are left to the caller
/// trailer, and type-only declarations run nothing their importers could reach.
fn window_declarations(file: &ExtractedFile, start: usize, end: usize) -> Vec<Declaration<'_>> {
    file.symbols
        .iter()
        .filter(|symbol| {
            !callable(symbol)
                && !matches!(symbol.kind.as_str(), "type" | "interface" | "trait")
                && symbol.flags.is_exported
                && symbol.owner.is_none()
                && symbol.range.start_line <= end
                && start <= symbol.range.end_line_inclusive()
                && is_top_level(file, symbol)
        })
        .take(DECLARATION_LIMIT)
        .map(|symbol| Declaration {
            symbol,
            importers: Vec::new(),
        })
        .collect()
}

/// Whether no function or type of the file encloses `symbol`.
fn is_top_level(file: &ExtractedFile, symbol: &ExtractedSymbol) -> bool {
    !file.symbols.iter().any(|other| {
        !std::ptr::eq(other, symbol)
            && (callable(other) || container(other))
            && other.range.start_line <= symbol.range.start_line
            && symbol.range.end_line_inclusive() <= other.range.end_line_inclusive()
            && other.range != symbol.range
    })
}

/// The declaration names `import` brings in, each with the lines of the importing file that
/// use it: calls of the imported binding or of its members, and recorded references. A named
/// import without a recorded use keeps its own line; a namespace import only counts the
/// members it uses.
fn imported_uses<'n>(
    navigation: &NavigationFile,
    import: &ImportEntry,
    by_name: &HashMap<&'n str, usize>,
) -> Vec<(&'n str, Vec<usize>)> {
    let binding = import.local_name.as_str();
    let is_outside_import = |line: &usize| {
        *line < import.range.start_line || import.range.end_line_inclusive() < *line
    };
    let mut found = Vec::new();
    match import.kind {
        ImportKind::Named => {
            let imported = import.imported_name.as_deref().unwrap_or(binding);
            let Some((&name, _)) = by_name.get_key_value(imported) else {
                return found;
            };
            let member_prefix = format!("{binding}.");
            let mut lines: Vec<usize> = navigation
                .calls
                .iter()
                .filter(|call| match call.receiver.as_deref() {
                    None => call.name == binding,
                    Some(receiver) => receiver == binding || receiver.starts_with(&member_prefix),
                })
                .map(|call| call.range.start_line)
                .chain(
                    navigation
                        .references
                        .iter()
                        .filter(|reference| reference.name == binding)
                        .map(|reference| reference.range.start_line),
                )
                .filter(is_outside_import)
                .collect();
            lines.sort_unstable();
            lines.dedup();
            if lines.is_empty() {
                lines.push(import.range.start_line);
            }
            found.push((name, lines));
        }
        ImportKind::Namespace => {
            let mut uses: HashMap<&'n str, Vec<usize>> = HashMap::new();
            for call in &navigation.calls {
                let Some(receiver) = call.receiver.as_deref() else {
                    continue;
                };
                // `ns.Name()` calls the declaration; `ns.Name.member()` uses it.
                let member = if receiver == binding {
                    Some(call.name.as_str())
                } else {
                    receiver
                        .strip_prefix(binding)
                        .and_then(|rest| rest.strip_prefix('.'))
                        .map(|rest| rest.split('.').next().unwrap_or(rest))
                };
                if let Some((&name, _)) = member.and_then(|member| by_name.get_key_value(member))
                {
                    if is_outside_import(&call.range.start_line) {
                        uses.entry(name).or_default().push(call.range.start_line);
                    }
                }
            }
            for (name, mut lines) in uses {
                lines.sort_unstable();
                lines.dedup();
                found.push((name, lines));
            }
        }
        ImportKind::Default | ImportKind::Glob => {}
    }
    found
}

/// Whether `import` in `importer` reaches declaration `name` of the file at `path`, directly
/// or through the re-exports of the module it names.
fn imports_from(
    resolver: &SourceResolver<'_>,
    importer: &ExtractedFile,
    import: &ImportEntry,
    name: &str,
    path: &str,
) -> bool {
    let Some(module) = import.source.as_deref() else {
        return false;
    };
    let language = crate::lang::spec_for_path(Path::new(&importer.file_path))
        .map(|spec| spec.language_name())
        .unwrap_or_default();
    let modules = match crate::implementations::family(language) {
        "script" => resolver
            .script_module(&importer.file_path, module)
            .into_iter()
            .collect(),
        "python" => resolver
            .python_module(&importer.file_path, module)
            .into_iter()
            .collect(),
        "dart" => resolver.dart_library_files(&importer.file_path, module),
        _ => Vec::new(),
    };
    modules.iter().any(|module| {
        resolver
            .export_origins(&module.file_path, name)
            .iter()
            .any(|origin| origin.file.file_path == path && origin.name == name)
    })
}

/// The trailer listing each declaration's importers, within `byte_cap`; `None` when no other
/// file imports any of them.
fn trailer_text(
    files: &[ExtractedFile],
    declarations: &[Declaration<'_>],
    is_partial: bool,
    byte_cap: usize,
) -> Option<String> {
    let reserve =
        omission_line(DECLARATION_LIMIT).len() + if is_partial { PARTIAL_LINE.len() } else { 0 };
    let mut text = String::from(HEADER);
    let mut omitted = 0;
    for declaration in declarations
        .iter()
        .filter(|declaration| !declaration.importers.is_empty())
    {
        let line = declaration_line(files, declaration);
        if omitted > 0 || text.len() + line.len() + reserve > byte_cap {
            omitted += 1;
            continue;
        }
        text.push_str(&line);
    }
    if text.len() == HEADER.len() {
        return None;
    }
    if omitted > 0 {
        text.push_str(&omission_line(omitted));
    }
    if is_partial {
        text.push_str(PARTIAL_LINE);
    }
    Some(text)
}

fn omission_line(count: usize) -> String {
    format!("\n- {count} more declaration(s) imported by other files (grep their names)")
}

/// `- client L12: a/user.ts: Panel.load L40, L52; b/api.ts: L3; +4 more files`
fn declaration_line(files: &[ExtractedFile], declaration: &Declaration<'_>) -> String {
    let symbol = declaration.symbol;
    let mut importers: Vec<_> = declaration.importers.iter().collect();
    importers.sort_by_key(|(file, _)| &files[*file].file_path);
    let shown: Vec<String> = importers
        .iter()
        .take(FILES_PER_DECLARATION)
        .map(|(file, lines)| file_places(&files[*file], lines))
        .collect();
    let mut line = format!(
        "\n- {} L{}: {}",
        symbol.name,
        symbol.range.start_line,
        shown.join("; ")
    );
    if importers.len() > FILES_PER_DECLARATION {
        line.push_str(&format!(
            "; +{} more files",
            importers.len() - FILES_PER_DECLARATION
        ));
    }
    line
}

/// `path: Owner.fn L40, L52` for the functions using the declaration, or its top-level lines
/// when the file records no use inside one.
fn file_places(file: &ExtractedFile, lines: &[usize]) -> String {
    let mut places: Vec<(String, Vec<usize>)> = Vec::new();
    for &line in lines {
        let label = enclosing_label(file, line);
        match places.iter_mut().find(|(place, _)| *place == label) {
            Some((_, lines)) => lines.push(line),
            None => places.push((label, vec![line])),
        }
    }
    if places.iter().any(|(label, _)| !label.is_empty()) {
        places.retain(|(label, _)| !label.is_empty());
    }
    let shown: Vec<String> = places
        .iter()
        .take(PLACES_PER_FILE)
        .map(|(label, lines)| {
            let numbers = lines
                .iter()
                .map(|number| format!("L{number}"))
                .collect::<Vec<_>>()
                .join(", ");
            if label.is_empty() {
                numbers
            } else {
                format!("{label} {numbers}")
            }
        })
        .collect();
    let mut text = format!("{}: {}", file.file_path, shown.join(", "));
    if places.len() > PLACES_PER_FILE {
        text.push_str(&format!(", +{} more places", places.len() - PLACES_PER_FILE));
    }
    text
}
