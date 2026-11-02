//! Importer trailer for `read view=source`: other files that import the window's declarations,
//! and where they use them. Callers reach functions by name, but objects, classes and constants
//! reach their users through imports, includes, packages and namespaces, which the index
//! records. Source rows above the trailer stay unchanged.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::shared_keys::enclosing_label;
use super::structure::callable;
use super::LiveOutput;
use crate::callers::resolution::{may_reach_unnamed, ImportReach, SourceResolver};
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
/// Time for scanning the index, resolving the matching imports and finding the uses. Importers
/// found by then are listed with a note that the lookup stopped.
const TRAILER_TIME_BUDGET: Duration = Duration::from_millis(100);
const PARTIAL_LINE: &str =
    "\n- import lookup stopped at its time limit; more importing files may exist (grep the name)";
/// Said instead of the trailer when the lookup stopped before finding any importing file, so
/// the stop does not read as "no other file imports these".
const TIME_LIMIT_LINE: &str = "[Importing files unavailable: the lookup reached its time limit before finding any; more may exist (grep the name).]";
pub(crate) const HEADER: &str =
    "[Other files importing this window's declarations (static analysis, approximate)]";

/// One declaration of the window, or the file as a whole, and the files importing it.
struct Declaration<'a> {
    /// `None` for the file itself, as a default, component or whole-file import binds it.
    symbol: Option<&'a ExtractedSymbol>,
    importers: Vec<Importer>,
}

/// One file importing a declaration.
struct Importer {
    /// Snapshot position of the importing file.
    file: usize,
    /// How the importing file spells a use: the bound name, `prefix.Name`, or the name itself.
    patterns: Vec<String>,
    /// Whether only a use in code shows that the file uses this declaration: the import
    /// reaches every declaration of the target, or none was needed.
    needs_use: bool,
    /// The import statement's line, shown when no use is found.
    import_line: Option<usize>,
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
    let position = files.iter().position(|file| file.file_path == path)?;
    let file = &files[position];
    // Stale or unindexed files would point at the wrong declarations.
    if super::diagnostics::unavailable(Some(file), anchor, &root, snapshot.flows().digest(path))
        .is_some()
    {
        return None;
    }
    let mut declarations = window_declarations(file, start, end);
    // The file itself is what a default or whole-file import binds; its first line stands for it.
    if start <= 1 {
        declarations.push(Declaration {
            symbol: None,
            importers: Vec::new(),
        });
    }
    if declarations.is_empty() {
        return None;
    }
    let deadline = started + TRAILER_TIME_BUDGET;
    let mut is_partial = false;
    let hints = TargetHints::new(path);
    // Imports that name a declaration, a module that may hold this file, or a namespace whose
    // recorded calls use a declaration's name; only these resolve.
    let names: Vec<&str> = declarations
        .iter()
        .filter_map(|declaration| declaration.symbol.map(|symbol| symbol.name.as_str()))
        .collect();
    let by_name: HashMap<&str, usize> = declarations
        .iter()
        .enumerate()
        .filter_map(|(index, declaration)| Some((declaration.symbol?.name.as_str(), index)))
        .collect();
    let whole_file = declarations
        .iter()
        .position(|declaration| declaration.symbol.is_none());
    let mut candidates = Vec::new();
    // Exports elsewhere that rename a declaration, or pass this file's default or namespace on:
    // their importers use the alias. `None` stands for every declaration, through a namespace.
    let mut renames = Vec::new();
    for (index, other) in files.iter().enumerate() {
        if index % 64 == 0 && Instant::now() >= deadline {
            is_partial = true;
            break;
        }
        if index == position {
            continue;
        }
        let Some(navigation) = &other.navigation else {
            continue;
        };
        let mut language = None;
        for import in &navigation.imports {
            let named = import
                .imported_name
                .as_deref()
                .filter(|_| import.kind == ImportKind::Named)
                .and_then(|name| by_name.get(name).copied());
            let may_name = hints.may_name(import) || namespace_uses_any(navigation, import, &names);
            if named.is_none() && !may_name {
                continue;
            }
            let language = *language.get_or_insert_with(|| {
                crate::lang::spec_for_path(Path::new(&other.file_path))
                    .map(|spec| spec.language_name())
                    .unwrap_or_default()
            });
            let is_unnamed = may_name && may_reach_unnamed(language, import);
            if named.is_some() || is_unnamed {
                candidates.push((index, import, named, is_unnamed));
            }
        }
        for export in &navigation.renamed_exports {
            let declaration = match export.imported_name.as_deref() {
                None if export.kind == ImportKind::Namespace => Some(None),
                Some("default") => whole_file.map(Some),
                Some(name) => by_name.get(name).map(|&declaration| Some(declaration)),
                None => None,
            };
            if let Some(declaration) = declaration {
                renames.push((index, export, declaration));
            }
        }
    }
    renames.retain(|(index, ..)| !filter.is_file_excluded(&files[*index].file_path));
    let resolver = SourceResolver::new(&files, &root);
    let mut aliases: HashMap<&str, Option<usize>> = HashMap::new();
    for (index, export, declaration) in renames {
        let barrel = files[index].file_path.as_str();
        let symbol = declaration.and_then(|declaration| declarations[declaration].symbol);
        let is_verified = match symbol {
            Some(symbol) => resolver
                .export_origins(barrel, &export.local_name)
                .iter()
                .any(|origin| origin.file.file_path == path && origin.name == symbol.name),
            // A passed-on default or namespace names this file as its module.
            None => export
                .source
                .as_deref()
                .and_then(|source| resolver.script_module(barrel, source))
                .is_some_and(|module| module.file_path == path),
        };
        if is_verified {
            aliases.insert(export.local_name.as_str(), declaration);
        }
    }
    if !aliases.is_empty() {
        for (index, other) in files.iter().enumerate() {
            if index == position {
                continue;
            }
            for import in other
                .navigation
                .iter()
                .flat_map(|navigation| &navigation.imports)
            {
                let alias = import
                    .imported_name
                    .as_deref()
                    .and_then(|name| aliases.get(name));
                if let (ImportKind::Named, Some(&declaration)) = (&import.kind, alias) {
                    candidates.push((index, import, declaration, declaration.is_none()));
                }
            }
        }
    }
    // Test files are left out, as by the caller trailer; only candidate files are checked.
    candidates.retain(|(index, ..)| !filter.is_file_excluded(&files[*index].file_path));
    let peers = resolver.package_peers(file);
    if candidates.is_empty() && peers.is_empty() {
        return is_partial.then(|| TIME_LIMIT_LINE.to_string());
    }
    // Imports naming a declaration resolve first: one check each, and the likeliest users.
    candidates.sort_by_key(|(_, _, _, is_unnamed)| *is_unnamed);
    'candidates: for (index, import, named, is_unnamed) in candidates {
        let importer = &files[index];
        let reachable: Vec<usize> = match named {
            Some(declaration) if !is_unnamed => vec![declaration],
            _ => (0..declarations.len()).collect(),
        };
        for declaration in reachable {
            if Instant::now() >= deadline {
                is_partial = true;
                break 'candidates;
            }
            let symbol = declarations[declaration].symbol;
            if let Some(reach) = resolver.import_reach(importer, import, file, symbol) {
                add_importer(
                    &mut declarations[declaration],
                    &files,
                    index,
                    reach,
                    Some(import),
                );
            }
        }
    }
    for index in peers {
        if filter.is_file_excluded(&files[index].file_path) {
            continue;
        }
        for declaration in declarations
            .iter_mut()
            .filter(|declaration| declaration.symbol.is_some())
        {
            add_importer(declaration, &files, index, ImportReach::Bare, None);
        }
    }
    let mut uses = Uses::new(&files, &root, deadline, &declarations);
    let shown = declarations
        .iter()
        .map(|declaration| uses.shown(path, declaration))
        .collect::<Vec<_>>();
    is_partial |= uses.is_past_deadline;
    trailer_text(&files, file, &declarations, &shown, is_partial, byte_cap)
}

/// Exported top-level values overlapping lines `start..=end`, each name once: importers reach
/// a name, not one of its assignments. Functions are left to the caller trailer where it runs,
/// except assembly labels and macros, which included files use by name rather than by calls;
/// type-only declarations run nothing their importers could reach.
fn window_declarations(file: &ExtractedFile, start: usize, end: usize) -> Vec<Declaration<'_>> {
    let spec = crate::lang::spec_for_path(Path::new(&file.file_path));
    let language = spec.map(|spec| spec.language_name()).unwrap_or_default();
    let lists_functions = language == "asm" || spec.is_some_and(|spec| !spec.caller_scan_enabled());
    let mut names = HashSet::new();
    file.symbols
        .iter()
        .filter(|symbol| {
            (lists_functions || !callable(symbol))
                && !matches!(symbol.kind.as_str(), "type" | "interface" | "trait")
                && is_importable(language, symbol)
                && symbol.owner.is_none()
                && symbol.range.start_line <= end
                && start <= symbol.range.end_line_inclusive()
                && is_top_level(file, symbol)
        })
        .filter(|symbol| names.insert(symbol.name.as_str()))
        .take(DECLARATION_LIMIT)
        .map(|symbol| Declaration {
            symbol: Some(symbol),
            importers: Vec::new(),
        })
        .collect()
}

/// Whether other files can import `symbol` by its name: an exported, plainly named
/// declaration, where shell names may hold hyphens. Files that include a header or source a
/// script see every top-level name, and a Starlark `load` every name without a leading
/// underscore.
fn is_importable(language: &str, symbol: &ExtractedSymbol) -> bool {
    let is_shell = matches!(language, "bash" | "zsh");
    let is_plain_name = !symbol.name.is_empty()
        && symbol
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$' || (c == '-' && is_shell));
    let is_visible = match language {
        "c" | "cpp" | "asm" | "bash" | "zsh" => true,
        "starlark" => !symbol.name.starts_with('_'),
        _ => symbol.flags.is_exported,
    };
    is_plain_name && is_visible
}

/// Whether no function or type of the file encloses `symbol`.
fn is_top_level(file: &ExtractedFile, symbol: &ExtractedSymbol) -> bool {
    !file.symbols.iter().any(|other| {
        !std::ptr::eq(other, symbol)
            && (callable(other) || super::structure::container(other))
            && other.range.start_line <= symbol.range.start_line
            && symbol.range.end_line_inclusive() <= other.range.end_line_inclusive()
            && other.range != symbol.range
    })
}

/// Records that file `index` reaches `declaration` through `reach`, merging with what other
/// imports of the same file reach. A file declaring the same top-level name uses its own
/// declaration under that bare name.
fn add_importer(
    declaration: &mut Declaration<'_>,
    files: &[ExtractedFile],
    index: usize,
    reach: ImportReach,
    import: Option<&ImportEntry>,
) {
    let name = declaration
        .symbol
        .map(|symbol| symbol.name.as_str())
        .unwrap_or_default();
    if reach == ImportReach::Bare
        && files[index].symbols.iter().any(|symbol| {
            symbol.name == name && symbol.owner.is_none() && is_top_level(&files[index], symbol)
        })
    {
        return;
    }
    let (pattern, needs_use) = match reach {
        ImportReach::Bound(local) => (local, false),
        ImportReach::Qualified(prefix) => (format!("{prefix}{name}"), true),
        ImportReach::Bare => (name.to_string(), true),
    };
    let import_line = import.map(|import| import.range.start_line);
    match declaration
        .importers
        .iter_mut()
        .find(|importer| importer.file == index)
    {
        Some(importer) => {
            if !importer.patterns.contains(&pattern) {
                importer.patterns.push(pattern);
            }
            if !needs_use {
                importer.needs_use = false;
                importer.import_line = importer.import_line.or(import_line);
            }
        }
        None => declaration.importers.push(Importer {
            file: index,
            patterns: vec![pattern],
            needs_use,
            import_line,
        }),
    }
}

/// Whether `import` binds a namespace whose recorded calls use one of `names` as a member, as
/// `ns.Name()` or `ns.Name.member()`.
fn namespace_uses_any(navigation: &NavigationFile, import: &ImportEntry, names: &[&str]) -> bool {
    let binding = import.local_name.as_str();
    import.kind == ImportKind::Namespace
        && navigation.calls.iter().any(|call| {
            let member = match call.receiver.as_deref() {
                Some(receiver) if receiver == binding => Some(call.name.as_str()),
                Some(receiver) => receiver
                    .strip_prefix(binding)
                    .and_then(|rest| rest.strip_prefix('.'))
                    .map(|rest| rest.split('.').next().unwrap_or(rest)),
                None => None,
            };
            member.is_some_and(|member| names.contains(&member))
        })
}

/// What a module source must end with to possibly name the read file: its file stem, or its
/// directory for package, namespace and directory imports. A module's entry file (`index`,
/// `mod`, `__init__`, `init`, `default`) is named by its directory alone.
struct TargetHints {
    stem: String,
    directory: String,
}

impl TargetHints {
    fn new(path: &str) -> Self {
        let path = Path::new(path);
        let text = |part: Option<&std::ffi::OsStr>| {
            part.and_then(|part| part.to_str())
                .unwrap_or_default()
                .to_string()
        };
        let directory = text(path.parent().and_then(Path::file_name));
        let stem = text(path.file_stem());
        let stem = match stem.to_ascii_lowercase().as_str() {
            "index" | "mod" | "__init__" | "init" | "default" => directory.clone(),
            _ => stem,
        };
        Self { stem, directory }
    }

    /// Whether `import` may name the read file: the last part of its module path, or the
    /// module it imports by name, is the read file's stem or directory.
    fn may_name(&self, import: &ImportEntry) -> bool {
        if import
            .imported_name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(&self.stem))
        {
            return true;
        }
        let Some(source) = import.source.as_deref() else {
            return false;
        };
        let (component, tail) = module_ending(source);
        [component, tail].iter().any(|ending| {
            ending.eq_ignore_ascii_case(&self.stem) || ending.eq_ignore_ascii_case(&self.directory)
        })
    }
}

/// The last component of a module path without a source file extension
/// (`./ui/user.service.ts` gives `user.service`), and that component's last dotted name
/// before any wildcard (`service`; `app.core._` gives `core`).
fn module_ending(source: &str) -> (&str, &str) {
    let source = source.trim_matches(['"', '\'', '`']);
    let component = source.rsplit(['/', '\\', ':']).next().unwrap_or(source);
    let component = match component.rsplit_once('.') {
        Some((name, extension)) if crate::lang::spec_for_ext(extension).is_some() => name,
        _ => component,
    };
    let tail = component
        .rsplit('.')
        .find(|part| !part.is_empty() && !matches!(*part, "_" | "*") && !part.starts_with('{'))
        .unwrap_or_default();
    (component, tail)
}

/// Use lines of importing files, found in their current source within the trailer's time.
struct Uses<'a> {
    files: &'a [ExtractedFile],
    root: &'a Path,
    deadline: Instant,
    /// Patterns wanted from each importing file, across the declarations.
    wanted: HashMap<usize, Vec<String>>,
    /// The current source of each file read, or `None` when it cannot be read or changed since
    /// indexing, so that its lines are unreliable.
    sources: HashMap<usize, Option<Rc<String>>>,
    /// Lines per pattern of each file searched, outside its import statements.
    found: HashMap<usize, HashMap<String, Vec<usize>>>,
    is_past_deadline: bool,
}

impl<'a> Uses<'a> {
    fn new(
        files: &'a [ExtractedFile],
        root: &'a Path,
        deadline: Instant,
        declarations: &[Declaration<'_>],
    ) -> Self {
        let mut wanted: HashMap<usize, Vec<String>> = HashMap::new();
        for importer in declarations
            .iter()
            .flat_map(|declaration| &declaration.importers)
        {
            let patterns = wanted.entry(importer.file).or_default();
            for pattern in &importer.patterns {
                if !pattern.is_empty() && !patterns.contains(pattern) {
                    patterns.push(pattern.clone());
                }
            }
        }
        Self {
            files,
            root,
            deadline,
            wanted,
            sources: HashMap::new(),
            found: HashMap::new(),
            is_past_deadline: false,
        }
    }

    /// The shown importers of `declaration`, nearest to `path` first, with the lines using
    /// it, and how many further files import it.
    fn shown(
        &mut self,
        path: &str,
        declaration: &Declaration<'_>,
    ) -> (Vec<(usize, Vec<usize>)>, usize) {
        let mut importers: Vec<&Importer> = declaration.importers.iter().collect();
        importers.sort_by_key(|importer| {
            let other = self.files[importer.file].file_path.as_str();
            (std::cmp::Reverse(shared_directories(path, other)), other)
        });
        let mut shown = Vec::new();
        let mut more = 0;
        for importer in importers {
            if shown.len() < FILES_PER_DECLARATION {
                match self.lines(importer) {
                    Some(lines) if !lines.is_empty() => shown.push((importer.file, lines)),
                    _ if !importer.needs_use => {
                        shown.push((importer.file, importer.import_line.into_iter().collect()))
                    }
                    _ => {}
                }
            } else if !importer.needs_use || self.mentions(importer) {
                more += 1;
            }
        }
        (shown, more)
    }

    /// The lines of the importing file using one of the importer's patterns in code, outside
    /// its import statements; `None` when its current source is unavailable or unreliable.
    fn lines(&mut self, importer: &Importer) -> Option<Vec<usize>> {
        let source = self.source(importer.file)?;
        let Self {
            files,
            wanted,
            found,
            ..
        } = self;
        let found = found.entry(importer.file).or_insert_with(|| {
            let file = &files[importer.file];
            let patterns = wanted.get(&importer.file).cloned().unwrap_or_default();
            let imports: Vec<(usize, usize)> = file
                .navigation
                .iter()
                .flat_map(|navigation| &navigation.imports)
                .map(|import| (import.range.start_line, import.range.end_line_inclusive()))
                .collect();
            let lines = crate::callers::code_use_lines(&file.file_path, &source, &patterns);
            let source_lines: Vec<&str> = source.lines().collect();
            patterns
                .into_iter()
                .zip(lines)
                .map(|(pattern, lines)| {
                    let lines = lines
                        .into_iter()
                        .filter(|line| {
                            !imports
                                .iter()
                                .any(|(start, end)| start <= line && line <= end)
                        })
                        .filter(|line| !is_shadowed(file, &source_lines, &pattern, *line))
                        .collect();
                    (pattern, lines)
                })
                .collect()
        });
        let mut lines: Vec<usize> = importer
            .patterns
            .iter()
            .filter_map(|pattern| found.get(pattern))
            .flatten()
            .copied()
            .collect();
        lines.sort_unstable();
        lines.dedup();
        Some(lines)
    }

    /// Whether the importing file spells one of the importer's patterns anywhere, without
    /// telling code from comments.
    fn mentions(&mut self, importer: &Importer) -> bool {
        self.source(importer.file).is_some_and(|source| {
            importer
                .patterns
                .iter()
                .any(|pattern| !pattern.is_empty() && source.contains(pattern.as_str()))
        })
    }

    /// The current source of file `index`, if it still holds what the index read.
    fn source(&mut self, index: usize) -> Option<Rc<String>> {
        if Instant::now() >= self.deadline {
            self.is_past_deadline = true;
            return None;
        }
        let (files, root) = (self.files, self.root);
        self.sources
            .entry(index)
            .or_insert_with(|| {
                let file = &files[index];
                let source = std::fs::read_to_string(root.join(&file.file_path)).ok()?;
                let indexed = file
                    .navigation
                    .as_ref()
                    .and_then(|navigation| navigation.implementations.as_ref())
                    .map(|facts| facts.source_digest.as_str());
                if indexed.is_some_and(|indexed| {
                    crate::implementations::digest(source.as_bytes()) != indexed
                }) {
                    return None;
                }
                Some(Rc::new(source))
            })
            .clone()
    }
}

/// The trailer listing each declaration's importers, within `byte_cap`; `None` when no other
/// file imports any of them, and a notice instead when the lookup stopped at its time limit
/// before finding any.
fn trailer_text(
    files: &[ExtractedFile],
    file: &ExtractedFile,
    declarations: &[Declaration<'_>],
    shown: &[(Vec<(usize, Vec<usize>)>, usize)],
    is_partial: bool,
    byte_cap: usize,
) -> Option<String> {
    let reserve =
        omission_line(DECLARATION_LIMIT).len() + if is_partial { PARTIAL_LINE.len() } else { 0 };
    let mut text = String::from(HEADER);
    let mut omitted = 0;
    for (declaration, (importers, more)) in declarations.iter().zip(shown) {
        if importers.is_empty() {
            continue;
        }
        let line = declaration_line(files, file, declaration, importers, *more);
        if omitted > 0 || text.len() + line.len() + reserve > byte_cap {
            omitted += 1;
            continue;
        }
        text.push_str(&line);
    }
    if text.len() == HEADER.len() {
        // A lookup cut short before it found anyone must not read as "no importing files".
        return (is_partial && omitted == 0).then(|| TIME_LIMIT_LINE.to_string());
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

/// `- registry L12: a/user.ts: Store.load L40, L52; b/api.ts: L3; +4 more files`, where the
/// file itself is named by its file name at line 1.
fn declaration_line(
    files: &[ExtractedFile],
    file: &ExtractedFile,
    declaration: &Declaration<'_>,
    importers: &[(usize, Vec<usize>)],
    more: usize,
) -> String {
    let (name, line) = match declaration.symbol {
        Some(symbol) => (symbol.name.as_str(), symbol.range.start_line),
        None => (
            Path::new(&file.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&file.file_path),
            1,
        ),
    };
    let shown: Vec<String> = importers
        .iter()
        .map(|(index, lines)| file_places(&files[*index], lines))
        .collect();
    let mut text = format!("\n- {name} L{line}: {}", shown.join("; "));
    if more > 0 {
        text.push_str(&format!("; +{more} more files"));
    }
    text
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
        text.push_str(&format!(
            ", +{} more places",
            places.len() - PLACES_PER_FILE
        ));
    }
    text
}

/// Whether a function enclosing `line` binds `name` itself, so that the name there is not the
/// imported one: it declares a local of that name, or, by its language's own rules, assigns or
/// takes a parameter of that name (Python, PowerShell) or declares it `local` (the shells, whose
/// functions otherwise assign the shell's variables).
fn is_shadowed(file: &ExtractedFile, lines: &[&str], name: &str, line: usize) -> bool {
    let language = crate::lang::spec_for_path(Path::new(&file.file_path))
        .map(|spec| spec.language_name())
        .unwrap_or_default();
    let within = |outer: &ExtractedSymbol, start: usize, end: usize| {
        outer.range.start_line <= start && end <= outer.range.end_line_inclusive()
    };
    file.symbols
        .iter()
        .filter(|function| callable(function) && within(function, line, line))
        .any(|function| {
            let declares = !matches!(language, "bash" | "zsh")
                && file.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && !std::ptr::eq(symbol, function)
                        && within(
                            function,
                            symbol.range.start_line,
                            symbol.range.end_line_inclusive(),
                        )
                });
            let body = lines
                .get(
                    function.range.start_line.saturating_sub(1)
                        ..function.range.end_line_inclusive(),
                )
                .unwrap_or_default();
            declares || binds_locally(language, body, name)
        })
}

/// Whether a function's lines bind `name` as its own variable by its language's syntax.
fn binds_locally(language: &str, lines: &[&str], name: &str) -> bool {
    match language {
        "python" => {
            python_parameters(lines)
                .iter()
                .any(|parameter| parameter == name)
                || lines.iter().any(|line| assigns(line, name, false))
        }
        "powershell" => {
            powershell_parameters(lines).any(|line| has_word(line, name, true))
                || lines.iter().any(|line| assigns(line, name, true))
        }
        "bash" | "zsh" => lines.iter().any(|line| {
            let line = line.trim_start();
            ["local", "declare", "typeset"].iter().any(|keyword| {
                line.strip_prefix(keyword).is_some_and(|rest| {
                    rest.starts_with([' ', '\t'])
                        && rest
                            .split(|c: char| c.is_whitespace() || c == '=')
                            .any(|word| word == name)
                })
            })
        }),
        _ => false,
    }
}

/// The parameter names of a Python function header, after any decorators: each name in its
/// parameter list before its type or default, without `*` or `**`.
fn python_parameters(lines: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    let mut depth = 0;
    let mut parameter = String::new();
    let header = lines.iter().skip_while(|line| {
        let line = line.trim_start();
        !(line.starts_with("def ") || line.starts_with("async def "))
    });
    for c in header.flat_map(|line| line.chars().chain(['\n'])) {
        match c {
            ')' | ']' | '}' | ',' if depth == 1 => {
                names.push(python_parameter_name(&parameter).to_string());
                parameter.clear();
                if c != ',' {
                    break;
                }
                continue;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth >= 1 && !(depth == 1 && c == '(') {
            parameter.push(c);
        }
    }
    names.retain(|name| !name.is_empty());
    names
}

/// `name` of a Python parameter written `*name: int = 0`.
fn python_parameter_name(parameter: &str) -> &str {
    let parameter = parameter.trim().trim_start_matches('*');
    parameter
        .split([':', '='])
        .next()
        .unwrap_or_default()
        .trim()
}

/// The lines declaring a PowerShell function's parameters: its header and a leading
/// `param (...)` block.
fn powershell_parameters<'a>(lines: &'a [&'a str]) -> impl Iterator<Item = &'a str> + 'a {
    let mut depth = 0i32;
    let mut in_block = false;
    let mut done = false;
    lines.iter().enumerate().filter_map(move |(index, line)| {
        if done {
            return None;
        }
        if index > 0 && !in_block {
            let trimmed = line.trim_start();
            in_block = trimmed.len() >= 5 && trimmed[..5].eq_ignore_ascii_case("param");
            if !in_block {
                // Only a leading `param` block declares parameters; other lines are the body.
                done = !(trimmed.is_empty() || trimmed.starts_with(['{', '#', '[']));
                return None;
            }
        }
        if in_block {
            depth += line.matches('(').count() as i32 - line.matches(')').count() as i32;
            done = depth <= 0 && line.contains(')');
        }
        Some(*line)
    })
}

/// Whether `line` assigns `name`, possibly after a type constraint: `name = ...`, `name += ...`.
fn assigns(line: &str, name: &str, ignore_case: bool) -> bool {
    let mut line = line.trim_start();
    while let Some(rest) = line.strip_prefix('[') {
        line = rest
            .split_once(']')
            .map_or("", |(_, rest)| rest)
            .trim_start();
    }
    let Some(head) = line.get(..name.len()) else {
        return false;
    };
    let is_name = if ignore_case {
        head.eq_ignore_ascii_case(name)
    } else {
        head == name
    };
    let rest = line[name.len()..].trim_start();
    is_name
        && ["=", "+=", "-=", "*=", "/=", "%=", "|=", "&=", ":="]
            .iter()
            .any(|operator| rest.starts_with(operator) && !rest[operator.len()..].starts_with('='))
}

/// Whether `line` spells `name` as a whole word.
fn has_word(line: &str, name: &str, ignore_case: bool) -> bool {
    let (line, name) = if ignore_case {
        (line.to_ascii_lowercase(), name.to_ascii_lowercase())
    } else {
        (line.to_string(), name.to_string())
    };
    let is_name_char = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(name.as_str()).any(|(at, _)| {
        !line[..at].chars().next_back().is_some_and(is_name_char)
            && !line[at + name.len()..]
                .chars()
                .next()
                .is_some_and(is_name_char)
    })
}

/// How many leading directories two paths share.
fn shared_directories(left: &str, right: &str) -> usize {
    let left = Path::new(left)
        .parent()
        .into_iter()
        .flat_map(Path::components);
    let right = Path::new(right)
        .parent()
        .into_iter()
        .flat_map(Path::components);
    left.zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functions_binding_a_name_themselves_shadow_it() {
        let python = [
            "@route(\"/x\", methods=[\"GET\"])",
            "def handle(limit: Dict[str, int] = None,",
            "           *rest, **options) -> Result:",
            "    return LIMIT",
        ];
        assert_eq!(python_parameters(&python), ["limit", "rest", "options"]);
        assert!(binds_locally("python", &python, "limit"));
        assert!(!binds_locally("python", &python, "Dict"));
        assert!(!binds_locally("python", &python, "LIMIT"));
        assert!(binds_locally(
            "python",
            &["def own():", "    LIMIT = 5"],
            "LIMIT"
        ));
        assert!(!binds_locally(
            "python",
            &["def own():", "    LIMIT == 5"],
            "LIMIT"
        ));
        let powershell = [
            "function Get-Level {",
            "  param (",
            "    [Settings] $Base",
            "  )",
            "  [int]$level = 1",
            "  return $Path",
            "}",
        ];
        assert!(binds_locally("powershell", &powershell, "$base"));
        assert!(binds_locally("powershell", &powershell, "$Level"));
        assert!(!binds_locally("powershell", &powershell, "$Path"));
        assert!(binds_locally(
            "bash",
            &["show() {", "  local LEVEL=1"],
            "LEVEL"
        ));
        assert!(!binds_locally("bash", &["show() {", "  LEVEL=1"], "LEVEL"));
    }

    #[test]
    fn a_lookup_stopped_before_finding_importers_says_so() {
        let file = ExtractedFile {
            file_path: "src/settings.ts".into(),
            total_lines: 1,
            symbols: Vec::new(),
            literals: Vec::new(),
            docstrings: Vec::new(),
            member_names: Vec::new(),
            navigation: None,
        };
        let declarations = [Declaration {
            symbol: None,
            importers: Vec::new(),
        }];
        let shown = [(Vec::new(), 0)];
        assert_eq!(
            trailer_text(&[], &file, &declarations, &shown, true, TRAILER_BYTE_CAP).as_deref(),
            Some(TIME_LIMIT_LINE)
        );
        assert_eq!(
            trailer_text(&[], &file, &declarations, &shown, false, TRAILER_BYTE_CAP),
            None
        );
    }
}
