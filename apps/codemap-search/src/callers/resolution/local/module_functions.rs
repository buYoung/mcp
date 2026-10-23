//! Calls of functions another file declares, reached only through source evidence: a Python
//! module import, a Kotlin import or package, a Dart library import, Ruby `require_relative`,
//! Lua `require`, PowerShell dot-sourcing, the Swift module, or a C/C++ header that the caller
//! and the defining file both include. A unique spelling alone is never a target, and a call
//! that a member of an implicit receiver could answer first stays unresolved.

use super::*;
use crate::implementations::{
    dart_library_imports, relative_path, same_source_root, DartLibraries,
};
use crate::parser::{ImportEntry, LocalBinding};

impl<'a> SourceResolver<'a> {
    pub(in crate::callers::resolution) fn module_function(
        &self,
        file: &'a ExtractedFile,
        call: &CallSite,
    ) -> Option<Target<'a>> {
        let language = crate::lang::spec_for_path(Path::new(&file.file_path))?.language_name();
        let source = self.source(&file.file_path)?;
        let navigation = source.extracted.navigation.as_ref()?;
        // Live extraction must still identify the same executable call.
        if !self.is_current_call(&file.file_path, &source, call) {
            return None;
        }
        let at = node_at(&source, &call.range)?;
        if member_may_answer(language, at) {
            return None;
        }
        // A parameter, a local or a declaration of this file under the same name (a field, a
        // type, an overload) takes precedence. Python methods are never reached by a bare name.
        if call.receiver.is_none()
            && (is_shadowed(&source, at, &call.name, language)
                || source.extracted.symbols.iter().any(|symbol| {
                    symbol.name == call.name
                        && !(language == "python" && symbol.owner.is_some())
                        && !(matches!(language, "c" | "cpp") && symbol.kind == "fn")
                }))
        {
            return None;
        }
        let imports = &navigation.imports;
        // Candidate files with the name declared there, and the Lua module table holding it.
        let (candidates, table): (Vec<(&'a ExtractedFile, String)>, Option<String>) =
            match (language, call.receiver.as_deref()) {
                ("python", None) => {
                    // A star import may rebind any name.
                    if imports
                        .iter()
                        .any(|import| import.local_name == "*" || import.kind == ImportKind::Glob)
                    {
                        return None;
                    }
                    let import = only_binding(imports, &call.name)??;
                    let name = import.imported_name.as_deref().unwrap_or(&call.name);
                    let module = self.python_module(&file.file_path, import.source.as_deref()?)?;
                    (self.exported(&module.file_path, name), None)
                }
                ("python", Some(receiver)) => {
                    let head = receiver.split('.').next().unwrap_or(receiver);
                    if has_binding(&source, at, head, None) {
                        return None;
                    }
                    let mut modules = imports
                        .iter()
                        .filter_map(|import| {
                            let module =
                                match (import.source.as_deref(), import.imported_name.as_deref()) {
                                    (Some(package), Some(name)) if package.ends_with('.') => {
                                        format!("{package}{name}")
                                    }
                                    (Some(package), Some(name)) => format!("{package}.{name}"),
                                    // `import pkg`
                                    (None, None) => import.local_name.clone(),
                                    _ => return None,
                                };
                            (import.local_name == receiver || module == receiver).then_some(module)
                        })
                        .collect::<Vec<_>>();
                    modules.sort();
                    modules.dedup();
                    let [module] = modules.as_slice() else {
                        return None;
                    };
                    let module = self.python_module(&file.file_path, module)?;
                    (self.exported(&module.file_path, &call.name), None)
                }
                ("kotlin", None) => {
                    // An explicit import outranks the declarations of the file's own package.
                    let (package, name, is_imported) = match only_binding(imports, &call.name)? {
                        Some(import) => (
                            import.source.clone()?,
                            import.imported_name.as_deref().unwrap_or(&call.name),
                            true,
                        ),
                        // Live resolution extracts no package, so both sides use the index.
                        None => (package_of(file)?, call.name.as_str(), false),
                    };
                    let files = self
                        .files
                        .iter()
                        .filter(|candidate| {
                            candidate.file_path.ends_with(".kt")
                                && package_of(candidate).as_deref() == Some(package.as_str())
                                && (is_imported
                                    || same_source_root(&file.file_path, &candidate.file_path))
                        })
                        .map(|candidate| (candidate, name.to_string()))
                        .collect();
                    (files, None)
                }
                ("dart", None) => {
                    // Library-private names are not imported, and the other parts of a library
                    // declare names that would take precedence.
                    let root = source.syntax.tree.root_node();
                    let mut cursor = root.walk();
                    let has_parts = root
                        .named_children(&mut cursor)
                        .any(|node| matches!(node.kind(), "part_directive" | "part_of_directive"));
                    if call.name.starts_with('_') || has_parts {
                        return None;
                    }
                    let libraries = dart_library_imports(root, source.text.as_bytes())
                        .into_iter()
                        .filter(|import| {
                            import.shown.is_empty() || import.shown.contains(&call.name)
                        })
                        .map(|import| import.uri)
                        .collect::<Vec<_>>();
                    let index = self.dart_libraries();
                    let files = self
                        .files
                        .iter()
                        .filter(|candidate| {
                            libraries.iter().any(|uri| {
                                index.matches(&file.file_path, uri, &candidate.file_path)
                            })
                        })
                        .flat_map(|library| self.exported(&library.file_path, &call.name))
                        .collect();
                    (files, None)
                }
                ("ruby", None) => (
                    self.relative_imports(&file.file_path, imports, &["rb"])
                        .into_iter()
                        .map(|candidate| (candidate, call.name.clone()))
                        .collect(),
                    None,
                ),
                ("powershell", None) => (
                    self.relative_imports(&file.file_path, imports, &["ps1", "psm1"])
                        .into_iter()
                        .map(|candidate| (candidate, call.name.clone()))
                        .collect(),
                    None,
                ),
                ("lua", Some(receiver)) => {
                    if !is_simple_identifier(receiver) || has_parameter(at, &source, receiver) {
                        return None;
                    }
                    // The nearest declaration of the receiver must be the `require` itself.
                    let import = only_binding(imports, receiver)??;
                    let binding = nearest_binding(&source, at, receiver)?;
                    if !(import.range.start_line <= binding.range.start_line
                        && binding.range.end_line <= import.range.end_line)
                    {
                        return None;
                    }
                    let module = self.lua_module(import.source.as_deref()?)?;
                    let table = returned_table(&*self.source(&module.file_path)?)?;
                    (vec![(module, call.name.clone())], Some(table))
                }
                ("swift", None) => {
                    let module = swift_module(&file.file_path);
                    let files = self
                        .files
                        .iter()
                        .filter(|candidate| {
                            candidate.file_path.ends_with(".swift")
                                && swift_module(&candidate.file_path) == module
                        })
                        .map(|candidate| (candidate, call.name.clone()))
                        .collect();
                    (files, None)
                }
                ("c" | "cpp", None) => {
                    let namespace = namespace_path(at, &source)?;
                    return self.included_definition(&file.file_path, &call.name, &namespace);
                }
                _ => return None,
            };
        let mut targets = Vec::new();
        for (target_file, name) in candidates {
            let Some(target_source) = self.source(&target_file.file_path) else {
                continue;
            };
            targets.extend(
                target_file
                    .symbols
                    .iter()
                    .filter(|symbol| {
                        symbol.kind == "fn"
                            && symbol.name == name
                            && match &table {
                                None => {
                                    symbol.owner.is_none()
                                        && is_top_level(target_file, symbol)
                                        && (target_file.file_path == file.file_path
                                            || !is_file_private(language, &target_source, symbol))
                                }
                                Some(table) => symbol.owner.as_deref() == Some(table.as_str()),
                            }
                            && current_symbol(&target_source, symbol)
                    })
                    .map(|symbol| Target {
                        file: target_file,
                        symbol,
                        is_precise: true,
                    }),
            );
        }
        let target = unique(targets)?;
        // Every top-level Ruby method shares one namespace, so another file may redefine it.
        if language == "ruby"
            && self
                .files
                .iter()
                .filter(|other| other.file_path.ends_with(".rb"))
                .flat_map(|other| other.symbols.iter().map(move |symbol| (other, symbol)))
                .filter(|(other, symbol)| {
                    symbol.kind == "fn"
                        && symbol.name == call.name
                        && symbol.owner.is_none()
                        && is_top_level(other, symbol)
                })
                .count()
                != 1
        {
            return None;
        }
        Some(target)
    }

    /// The declarations the module file at `path` exports as `name`, with their declared names.
    fn exported(&self, path: &str, name: &str) -> Vec<(&'a ExtractedFile, String)> {
        self.export_origins(path, name)
            .iter()
            .map(|origin| (origin.file, origin.name.clone()))
            .collect()
    }

    /// Indexed Dart libraries that an import or export URI in the file at `path` names.
    pub(crate) fn dart_library_files(&self, path: &str, uri: &str) -> Vec<&'a ExtractedFile> {
        let libraries = self.dart_libraries();
        self.files
            .iter()
            .filter(|file| libraries.matches(path, uri, &file.file_path))
            .collect()
    }

    /// Indexed Dart libraries by their path below `lib/`.
    fn dart_libraries(&self) -> Rc<DartLibraries> {
        self.dart_libraries
            .borrow_mut()
            .get_or_insert_with(|| {
                Rc::new(DartLibraries::new(
                    self.files.iter().map(|file| file.file_path.as_str()),
                ))
            })
            .clone()
    }

    /// The one indexed file a dotted Python module names: relative to the importing package
    /// for leading dots, otherwise below a directory that is not itself a package.
    pub(crate) fn python_module(&self, importer: &str, module: &str) -> Option<&'a ExtractedFile> {
        let levels = module.chars().take_while(|c| *c == '.').count();
        let path = module[levels..].replace('.', "/");
        let names = if path.is_empty() {
            vec!["__init__.py".to_string()]
        } else {
            vec![format!("{path}.py"), format!("{path}/__init__.py")]
        };
        let matches = if levels > 0 {
            let up = "../".repeat(levels - 1);
            names
                .iter()
                .filter_map(|name| relative_path(importer, &format!("{up}{name}")))
                .filter_map(|path| self.files_by_path.get(path.as_str()).copied())
                .collect::<Vec<_>>()
        } else {
            self.files
                .iter()
                .filter(|file| {
                    names.iter().any(|name| {
                        let Some(root) = file.file_path.strip_suffix(name.as_str()) else {
                            return false;
                        };
                        root.is_empty()
                            || root.strip_suffix('/').is_some_and(|root| {
                                !self
                                    .files_by_path
                                    .contains_key(format!("{root}/__init__.py").as_str())
                            })
                    })
                })
                .collect()
        };
        let [only] = matches.as_slice() else {
            return None;
        };
        Some(*only)
    }

    /// The one indexed file a dotted Lua module names, as `a/b.lua` or `a/b/init.lua`.
    fn lua_module(&self, module: &str) -> Option<&'a ExtractedFile> {
        let path = module.replace('.', "/");
        let names = [format!("{path}.lua"), format!("{path}/init.lua")];
        let matches = self
            .files
            .iter()
            .filter(|file| {
                names.iter().any(|name| {
                    file.file_path == *name || file.file_path.ends_with(&format!("/{name}"))
                })
            })
            .collect::<Vec<_>>();
        let [only] = matches.as_slice() else {
            return None;
        };
        Some(*only)
    }

    /// Files loaded by a path relative to the calling file: Ruby `require_relative` or
    /// PowerShell dot-sourcing and `Import-Module`.
    fn relative_imports(
        &self,
        path: &str,
        imports: &[ImportEntry],
        extensions: &[&str],
    ) -> Vec<&'a ExtractedFile> {
        imports
            .iter()
            .filter_map(|import| import.source.as_deref())
            .map(script_relative)
            .filter(|module| module.starts_with("./") || module.starts_with("../"))
            .filter_map(|module| relative_path(path, &module))
            .flat_map(|module| {
                let named = extensions
                    .iter()
                    .map(|extension| format!("{module}.{extension}"))
                    .collect::<Vec<_>>();
                std::iter::once(module).chain(named)
            })
            .filter_map(|candidate| self.files_by_path.get(candidate.as_str()).copied())
            .collect()
    }

    /// The one external definition of `name` behind a header the caller includes: the header
    /// declares it in the caller's namespace and the defining file includes that header too.
    /// Includes are followed through other headers, as the preprocessor does: a file that
    /// includes only an umbrella header still reaches the declaring header.
    fn included_definition(&self, path: &str, name: &str, namespace: &str) -> Option<Target<'a>> {
        let lookups = self.module_lookups();
        let functions = lookups.functions.get(name)?;
        let headers = self.included_headers(path);
        let reachable = self.reachable_headers(path);
        let declaring: Vec<&'a ExtractedFile> = reachable
            .iter()
            .copied()
            .filter(|header| {
                functions.iter().any(|(file, symbol)| {
                    std::ptr::eq(*header, *file)
                        && self.is_free_function(file, symbol, namespace, false)
                })
            })
            .collect();
        if declaring.is_empty() {
            return None;
        }
        let is_declared_directly = headers.iter().any(|header| {
            declaring
                .iter()
                .any(|declared| std::ptr::eq(*declared, *header))
        });
        let mut targets = Vec::new();
        for &(candidate, symbol) in functions {
            if !self.is_free_function(candidate, symbol, namespace, true) {
                continue;
            }
            let included = self.included_headers(&candidate.file_path);
            let reached = self.reachable_headers(&candidate.file_path);
            let shares_direct_header = is_declared_directly
                && headers.iter().any(|header| {
                    std::ptr::eq(*header, candidate)
                        || included.iter().any(|other| std::ptr::eq(*other, *header))
                });
            let reaches_declaration = declaring.iter().any(|header| {
                std::ptr::eq(*header, candidate)
                    || reached.iter().any(|other| std::ptr::eq(*other, *header))
            });
            if shares_direct_header || reaches_declaration {
                targets.push(Target {
                    file: candidate,
                    symbol,
                    is_precise: true,
                });
            }
        }
        unique(targets)
    }

    /// The one definition that the C/C++ function prototype `symbol` of `path` stands for: a
    /// definition of the same name and namespace in `path`, or one with external linkage in a
    /// file that includes `path`. `None` for a definition or when several match.
    pub(crate) fn prototype_definition(
        &self,
        path: &str,
        symbol: &ExtractedSymbol,
    ) -> Option<(&'a ExtractedFile, &'a ExtractedSymbol)> {
        let language = crate::lang::spec_for_path(Path::new(path))?.language_name();
        if !matches!(language, "c" | "cpp") || symbol.kind != "fn" || symbol.owner.is_some() {
            return None;
        }
        let declaring = *self.files_by_path.get(path)?;
        let source = self.source(path)?;
        let node = node_at(&source, &symbol.range)?;
        if node.kind() == "function_definition" || !current_symbol(&source, symbol) {
            return None;
        }
        let namespace = namespace_path(node, &source)?;
        let lookups = self.module_lookups();
        let mut definitions = Vec::new();
        for &(file, definition) in lookups.functions.get(symbol.name.as_str())? {
            let is_same_file = std::ptr::eq(file, declaring);
            let is_definition = if is_same_file {
                self.source(path).is_some_and(|source| {
                    node_at(&source, &definition.range).is_some_and(|node| {
                        node.kind() == "function_definition"
                            && namespace_path(node, &source).as_deref() == Some(namespace.as_str())
                    })
                })
            } else {
                self.is_free_function(file, definition, &namespace, true)
                    && self
                        .reachable_headers(&file.file_path)
                        .iter()
                        .any(|header| std::ptr::eq(*header, declaring))
            };
            if is_definition {
                definitions.push((file, definition));
            }
        }
        match definitions.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    /// Whether `symbol`, a top-level function of `file`, is still declared in `namespace`; with
    /// `is_definition`, whether it is a definition with external linkage.
    fn is_free_function(
        &self,
        file: &ExtractedFile,
        symbol: &ExtractedSymbol,
        namespace: &str,
        is_definition: bool,
    ) -> bool {
        (!is_definition || symbol.flags.is_exported)
            && self.source(&file.file_path).is_some_and(|source| {
                current_symbol(&source, symbol)
                    && node_at(&source, &symbol.range).is_some_and(|node| {
                        (!is_definition || node.kind() == "function_definition")
                            && namespace_path(node, &source).as_deref() == Some(namespace)
                    })
            })
    }

    /// Whether live extraction of `path`, held by `source`, still records `call`.
    fn is_current_call(&self, path: &str, source: &Source, call: &CallSite) -> bool {
        let lookups = self.module_lookups();
        let positions = Rc::clone(
            lookups
                .call_positions
                .borrow_mut()
                .entry(path.to_string())
                .or_insert_with(|| {
                    let mut positions = CallPositions::new();
                    for (index, current) in source
                        .extracted
                        .navigation
                        .iter()
                        .flat_map(|navigation| &navigation.calls)
                        .enumerate()
                    {
                        positions
                            .entry((current.range.start_line, current.range.start_col))
                            .or_default()
                            .push(index);
                    }
                    Rc::new(positions)
                }),
        );
        let calls = source
            .extracted
            .navigation
            .as_ref()
            .map(|navigation| navigation.calls.as_slice())
            .unwrap_or_default();
        positions
            .get(&(call.range.start_line, call.range.start_col))
            .into_iter()
            .flatten()
            .any(|&index| calls.get(index) == Some(call))
    }

    /// Indexed headers named by the includes of `path`: beside the including file, else the one
    /// file whose path ends with the included path.
    fn included_headers(&self, path: &str) -> Rc<Vec<&'a ExtractedFile>> {
        let lookups = self.module_lookups();
        if let Some(headers) = lookups.headers.borrow().get(path) {
            return Rc::clone(headers);
        }
        let source = self.source(path);
        let imports = source
            .as_ref()
            .and_then(|source| source.extracted.navigation.as_ref())
            .map(|navigation| navigation.imports.as_slice())
            .unwrap_or_default();
        let headers = Rc::new(self.resolve_includes(&lookups, path, imports));
        lookups
            .headers
            .borrow_mut()
            .insert(path.to_string(), Rc::clone(&headers));
        headers
    }

    /// Indexed headers that `path` includes directly or through other headers, nearest first.
    /// Headers below the first level use their indexed includes, so following an umbrella
    /// header does not reparse every header it reaches.
    fn reachable_headers(&self, path: &str) -> Rc<Vec<&'a ExtractedFile>> {
        let lookups = self.module_lookups();
        if let Some(headers) = lookups.reachable.borrow().get(path) {
            return Rc::clone(headers);
        }
        let mut reached: Vec<&'a ExtractedFile> = Vec::new();
        let mut queue: std::collections::VecDeque<&'a ExtractedFile> =
            self.included_headers(path).iter().copied().collect();
        while let Some(header) = queue.pop_front() {
            if reached.len() >= REACHABLE_HEADERS_MAX {
                break;
            }
            if reached.iter().any(|seen| std::ptr::eq(*seen, header)) {
                continue;
            }
            reached.push(header);
            let imports = header
                .navigation
                .as_ref()
                .map(|navigation| navigation.imports.as_slice())
                .unwrap_or_default();
            queue.extend(self.resolve_includes(&lookups, &header.file_path, imports));
        }
        let reached = Rc::new(reached);
        lookups
            .reachable
            .borrow_mut()
            .insert(path.to_string(), Rc::clone(&reached));
        reached
    }

    /// The indexed files that the includes of `path` name: beside the including file, else
    /// the one file whose path ends with the included path.
    fn resolve_includes(
        &self,
        lookups: &ModuleLookups<'a>,
        path: &str,
        imports: &[ImportEntry],
    ) -> Vec<&'a ExtractedFile> {
        let mut headers = Vec::new();
        for include in imports.iter().filter_map(|import| import.source.as_deref()) {
            if let Some(beside) = relative_path(path, include)
                .and_then(|candidate| self.files_by_path.get(candidate.as_str()).copied())
            {
                headers.push(beside);
                continue;
            }
            let file_name = include.rsplit('/').next().unwrap_or(include);
            let matches = lookups
                .by_file_name
                .get(file_name)
                .into_iter()
                .flatten()
                .filter(|file| file.file_path.ends_with(&format!("/{include}")))
                .collect::<Vec<_>>();
            if let [only] = matches.as_slice() {
                headers.push(**only);
            }
        }
        headers
    }

    /// Lookups shared by the calls one resolver checks, built on first use.
    fn module_lookups(&self) -> Rc<ModuleLookups<'a>> {
        self.module_lookups
            .borrow_mut()
            .get_or_insert_with(|| {
                let mut lookups = ModuleLookups::default();
                for file in self.files {
                    if let Some(name) = Path::new(&file.file_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                    {
                        lookups.by_file_name.entry(name).or_default().push(file);
                    }
                    if matches!(
                        crate::lang::spec_for_path(Path::new(&file.file_path))
                            .map(|spec| spec.language_name()),
                        Some("c" | "cpp")
                    ) {
                        for symbol in file
                            .symbols
                            .iter()
                            .filter(|symbol| symbol.kind == "fn" && symbol.owner.is_none())
                        {
                            lookups
                                .functions
                                .entry(symbol.name.as_str())
                                .or_default()
                                .push((file, symbol));
                        }
                    }
                }
                Rc::new(lookups)
            })
            .clone()
    }
}

/// Lookups for calls into other files, shared by the calls one resolver checks.
#[derive(Default)]
pub(in crate::callers::resolution) struct ModuleLookups<'a> {
    /// Indexed files by file name, to find an included path by its suffix.
    by_file_name: HashMap<&'a str, Vec<&'a ExtractedFile>>,
    /// Top-level C/C++ function declarations and definitions, by name.
    functions: HashMap<&'a str, Vec<(&'a ExtractedFile, &'a ExtractedSymbol)>>,
    /// The headers each file includes.
    headers: RefCell<HashMap<String, Rc<Vec<&'a ExtractedFile>>>>,
    /// The headers each file reaches through includes.
    reachable: RefCell<HashMap<String, Rc<Vec<&'a ExtractedFile>>>>,
    /// Indices of the live calls of each file, by start position.
    call_positions: RefCell<HashMap<String, Rc<CallPositions>>>,
}

/// Indices into a file's live calls by start line and column.
type CallPositions = HashMap<(usize, usize), Vec<usize>>;

/// Headers one file's includes are followed to at most.
const REACHABLE_HEADERS_MAX: usize = 512;

/// Whether a member of an implicit receiver could answer an unqualified call at `at` before a
/// function of another file. Kotlin, Swift, Ruby and C++ look up members first inside a type
/// or member definition; Kotlin lambdas and extension functions may bind a receiver, and Ruby
/// DSLs evaluate blocks against other objects. Dart looks up lexical scope first.
fn member_may_answer(language: &str, at: Node) -> bool {
    let mut ancestors = std::iter::successors(at.parent(), |node| node.parent());
    match language {
        "kotlin" => ancestors.any(|node| {
            matches!(
                node.kind(),
                "class_declaration"
                    | "object_declaration"
                    | "object_literal"
                    | "lambda_literal"
                    | "anonymous_function"
            ) || node.kind() == "function_declaration" && has_receiver_type(node)
        }),
        "swift" => class_scope(at).is_some(),
        "ruby" => ancestors.any(|node| {
            matches!(
                node.kind(),
                "class"
                    | "module"
                    | "singleton_class"
                    | "singleton_method"
                    | "block"
                    | "do_block"
                    | "lambda"
            )
        }),
        "cpp" => {
            class_scope(at).is_some()
                || ancestors
                    .find(|node| node.kind() == "function_definition")
                    .is_some_and(is_qualified_definition)
        }
        _ => false,
    }
}

/// Whether a Kotlin function declares a receiver type, as `fun String.trimmed()` does.
fn has_receiver_type(function: Node) -> bool {
    let name = function.child_by_field_name("name").map(|node| node.id());
    let mut cursor = function.walk();
    let found = function
        .named_children(&mut cursor)
        .take_while(|child| Some(child.id()) != name)
        .any(|child| {
            !matches!(child.kind(), "modifiers" | "type_parameters")
                && !child.kind().ends_with("comment")
        });
    found
}

/// Whether a C++ function definition names a member or another scope, as `Type::method`
/// does. An unrecognized declarator shape counts as qualified.
fn is_qualified_definition(definition: Node) -> bool {
    let mut declarator = definition.child_by_field_name("declarator");
    while let Some(node) = declarator {
        if node.kind() == "identifier" {
            return false;
        }
        declarator = node.child_by_field_name("declarator");
    }
    true
}

/// The namespaces enclosing `node`, outermost first and joined by `::`; `None` inside an
/// anonymous namespace, whose members no other file can call.
fn namespace_path(node: Node, source: &Source) -> Option<String> {
    let mut names = Vec::new();
    for ancestor in std::iter::successors(node.parent(), |node| node.parent()) {
        if ancestor.kind() == "namespace_definition" {
            let name = text(ancestor.child_by_field_name("name")?, source)?;
            names.push(name.split_whitespace().collect::<String>());
        }
    }
    names.reverse();
    Some(names.join("::"))
}

/// A parameter or a local declaration that binds `name` at `at`. A C/C++ declaration outside
/// any function body is a prototype of the called function itself.
fn is_shadowed(source: &Source, at: Node, name: &str, language: &str) -> bool {
    if !matches!(language, "c" | "cpp") {
        return has_binding(source, at, name, None);
    }
    let mut enclosing = std::iter::successors(at.parent(), |node| node.parent());
    let has_parameter = enclosing.any(|function| {
        if !matches!(function.kind(), "function_definition" | "lambda_expression") {
            return false;
        }
        let mut declarator = function.child_by_field_name("declarator");
        while let Some(node) = declarator {
            if let Some(parameters) = node.child_by_field_name("parameters") {
                return has_identifier(parameters, source, name);
            }
            declarator = node.child_by_field_name("declarator");
        }
        false
    });
    has_parameter
        || source
            .extracted
            .navigation
            .as_ref()
            .is_some_and(|navigation| {
                navigation
                    .local_bindings
                    .iter()
                    .filter(|binding| binding.name == name)
                    .any(|binding| {
                        node_at(source, &binding.range).is_some_and(|node| {
                            std::iter::successors(node.parent(), |node| node.parent())
                                .find(|node| node.kind() == "compound_statement")
                                .is_some_and(|block| same_tree(node, at) && contains(block, at))
                        })
                    })
            })
}

/// The import binding `name` when every import of that name agrees on what it binds: `None`
/// for disagreeing imports, `Some(None)` when no import binds it.
pub(super) fn only_binding<'i>(
    imports: &'i [ImportEntry],
    name: &str,
) -> Option<Option<&'i ImportEntry>> {
    let mut bound = imports.iter().filter(|import| import.local_name == name);
    let Some(first) = bound.next() else {
        return Some(None);
    };
    bound
        .all(|other| other.source == first.source && other.imported_name == first.imported_name)
        .then_some(Some(first))
}

/// The nearest declaration of `name` visible at `at`.
fn nearest_binding<'s>(source: &'s Source, at: Node, name: &str) -> Option<&'s LocalBinding> {
    source
        .extracted
        .navigation
        .as_ref()?
        .local_bindings
        .iter()
        .filter(|binding| binding.name == name)
        .filter_map(|binding| {
            let node = node_at(source, &binding.range)?;
            let scope = lexical_scope(node);
            (same_tree(node, at) && node.start_byte() <= at.start_byte() && contains(scope, at))
                .then(|| {
                    (
                        (
                            scope.end_byte() - scope.start_byte(),
                            std::cmp::Reverse(node.start_byte()),
                        ),
                        binding,
                    )
                })
        })
        .min_by_key(|(key, _)| *key)
        .map(|(_, binding)| binding)
}

/// The local table a Lua module file returns, as in `return M`.
fn returned_table(source: &Source) -> Option<String> {
    let root = source.syntax.tree.root_node();
    let mut cursor = root.walk();
    let statement = root
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "return_statement")
        .last()?;
    let mut cursor = statement.walk();
    let values = statement
        .named_children(&mut cursor)
        .find(|node| node.kind() == "expression_list")?;
    let mut cursor = values.walk();
    let values = values.named_children(&mut cursor).collect::<Vec<_>>();
    let [value] = values.as_slice() else {
        return None;
    };
    (value.kind() == "identifier")
        .then(|| text(*value, source).map(str::to_string))
        .flatten()
}

/// The package a Kotlin file declares.
fn package_of(file: &ExtractedFile) -> Option<String> {
    file.navigation
        .as_ref()?
        .implementations
        .as_ref()?
        .units
        .iter()
        .map(|unit| unit.namespace.clone())
        .find(|namespace| !namespace.is_empty())
}

/// A SwiftPM module is the directory below `Sources/<Module>/`; elsewhere the directory.
fn swift_module(path: &str) -> String {
    if let Some((prefix, rest)) = path.split_once("Sources/") {
        if let Some((module, _)) = rest.split_once('/') {
            return format!("{prefix}Sources/{module}");
        }
    }
    Path::new(path)
        .parent()
        .map(|directory| directory.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A script path written relative to the calling file. `$PSScriptRoot` is the directory of
/// the calling script.
fn script_relative(module: &str) -> String {
    let module = module.trim().trim_matches(['"', '\'']).replace('\\', "/");
    for root in ["$PSScriptRoot", "${PSScriptRoot}"] {
        if module
            .get(..root.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(root))
        {
            return format!(".{}", &module[root.len()..]);
        }
    }
    module
}

/// Whether no other declaration of the file encloses `symbol`.
pub(super) fn is_top_level(file: &ExtractedFile, symbol: &ExtractedSymbol) -> bool {
    !file.symbols.iter().any(|other| {
        !std::ptr::eq(other, symbol)
            && (other.kind == "fn" || crate::declarations::container(other))
            && other.range.start_line <= symbol.range.start_line
            && symbol.range.end_line_inclusive() <= other.range.end_line_inclusive()
            && other.range != symbol.range
    })
}

/// Whether a top-level function is hidden from the other files of its module: Swift
/// `private`/`fileprivate`, Kotlin `private`, or a Dart library-private name.
fn is_file_private(language: &str, source: &Source, symbol: &ExtractedSymbol) -> bool {
    match language {
        "swift" => node_at(source, &symbol.range)
            .and_then(|node| text(node, source))
            .and_then(|declaration| declaration.split_once("func"))
            .is_some_and(|(modifiers, _)| {
                modifiers
                    .split_whitespace()
                    .any(|word| matches!(word, "private" | "fileprivate"))
            }),
        "kotlin" => !symbol.flags.is_exported,
        "dart" => symbol.name.starts_with('_'),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Files of one workspace, the calling file and line, the call name and its target.
    type Case<'a> = (&'a [(&'a str, &'a str)], &'a str, usize, &'a str, &'a str);

    fn check(cases: &[Case<'_>]) {
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        for (entries, caller, line, name, expected) in cases {
            assert_eq!(
                targets(entries, caller, *line, name),
                [*expected],
                "{caller}:{line} {name}"
            );
        }
    }

    #[test]
    fn imported_functions_of_other_files_resolve_in_each_language() {
        check(&[
            (&[("src/pkg/util.py", "def helper():\n    pass\n"), ("app/main.py", "from pkg.util import helper\n\ndef run():\n    helper()\n")], "app/main.py", 4, "helper", "src/pkg/util.py:1"),
            (&[("app/util.py", "def helper():\n    pass\n"), ("app/main.py", "from .util import helper as h\nh()\n")], "app/main.py", 2, "h", "app/util.py:1"),
            (&[("pkg/util.py", "def helper():\n    pass\n"), ("main.py", "from pkg import util\nutil.helper()\n")], "main.py", 2, "helper", "pkg/util.py:1"),
            (&[("pkg/util.py", "def helper():\n    pass\n"), ("main.py", "import pkg.util\npkg.util.helper()\n")], "main.py", 2, "helper", "pkg/util.py:1"),
            (&[("app/util/Util.kt", "package app.util\n\nfun helper() = 1\n"), ("app/web/Web.kt", "package app.web\n\nimport app.util.helper\n\nfun run() { helper() }\n")], "app/web/Web.kt", 5, "helper", "app/util/Util.kt:3"),
            (&[("src/main/kotlin/app/A.kt", "package app\n\nfun run() { helper() }\n"), ("src/main/kotlin/app/B.kt", "package app\n\nfun helper() = 1\n")], "src/main/kotlin/app/A.kt", 3, "helper", "src/main/kotlin/app/B.kt:3"),
            // Dart looks up imported names before inherited members.
            (&[("lib/util.dart", "int helper() => 1;\n"), ("lib/api.dart", "import 'util.dart';\n\nclass Api extends Base {\n  void run() { helper(); }\n}\n")], "lib/api.dart", 4, "helper", "lib/util.dart:1"),
            (&[("lib/util.dart", "int helper() => 1;\n"), ("lib/api.dart", "import 'util.dart' show helper;\nvoid run() { helper(); }\n")], "lib/api.dart", 2, "helper", "lib/util.dart:1"),
            // A `package:` URI names the only indexed `lib/` path, or with several workspace
            // packages, the one in the directory named after the package.
            (&[("mobile/lib/util.dart", "int helper() => 1;\n"), ("mobile/lib/api.dart", "import 'package:my_app/util.dart';\nvoid run() { helper(); }\n")], "mobile/lib/api.dart", 2, "helper", "mobile/lib/util.dart:1"),
            (&[("packages/a/lib/util.dart", "int helper() => 1;\n"), ("packages/b/lib/util.dart", "int helper() => 2;\n"), ("app/lib/api.dart", "import 'package:a/util.dart';\nvoid run() { helper(); }\n")], "app/lib/api.dart", 2, "helper", "packages/a/lib/util.dart:1"),
            (&[("lib/helpers.rb", "def helper(x)\n  x\nend\n"), ("lib/run.rb", "require_relative 'helpers'\n\ndef run\n  helper(1)\nend\n")], "lib/run.rb", 4, "helper", "lib/helpers.rb:1"),
            (&[("app/store.lua", "local M = {}\nlocal H = {}\nfunction M.save(x) return x end\nfunction H.save(x) return x end\nreturn M\n"), ("use.lua", "local store = require(\"app.store\")\nstore.save(1)\n")], "use.lua", 2, "save", "app/store.lua:3"),
            (&[("s/helpers.ps1", "function Get-Thing { param($x) $x }\n"), ("s/main.ps1", ". \"$PSScriptRoot\\helpers.ps1\"\nGet-Thing -x 1\n")], "s/main.ps1", 2, "Get-Thing", "s/helpers.ps1:1"),
            (&[("Sources/App/Util.swift", "func helper() -> Int { 1 }\n"), ("Sources/App/Main.swift", "func run() { _ = helper() }\n")], "Sources/App/Main.swift", 1, "helper", "Sources/App/Util.swift:1"),
            (&[("include/util.h", "int add(int a, int b);\n"), ("src/util.c", "#include \"util.h\"\nint add(int a, int b) { return a + b; }\n"), ("src/main.c", "#include \"util.h\"\nint main(void) { int x = add(1, 2); return x; }\n")], "src/main.c", 2, "add", "src/util.c:2"),
            (&[("math.h", "namespace m {\nint add(int a, int b);\n}\n"), ("math.cpp", "#include \"math.h\"\nnamespace m {\nint add(int a, int b) { return a + b; }\n}\n"), ("main.cpp", "#include \"math.h\"\nnamespace m {\nint run() { return add(1, 2); }\n}\n")], "main.cpp", 3, "add", "math.cpp:3"),
        ]);
    }

    #[test]
    fn calls_that_imports_do_not_settle_stay_unresolved() {
        check(&[
            // Two imports bind the name, two files hold the module, a star import may rebind
            // the name, or the module file sits inside a package rather than at a root.
            (&[("a.py", "def f():\n    pass\n"), ("b.py", "def f():\n    pass\n"), ("main.py", "try:\n    from a import f\nexcept ImportError:\n    from b import f\nf()\n")], "main.py", 5, "f", "-"),
            (&[("one/pkg/util.py", "def helper():\n    pass\n"), ("two/pkg/util.py", "x = 1\n"), ("main.py", "from pkg.util import helper\nhelper()\n")], "main.py", 2, "helper", "-"),
            (&[("pkg/util.py", "def helper():\n    pass\n"), ("pkg/more.py", "def other():\n    pass\n"), ("main.py", "from pkg.util import helper\nfrom pkg.more import *\nhelper()\n")], "main.py", 3, "helper", "-"),
            (&[("tests/__init__.py", "\n"), ("tests/json.py", "def loads():\n    pass\n"), ("main.py", "from json import loads\nloads()\n")], "main.py", 2, "loads", "-"),
            // Members of a class, a lambda receiver or an extension receiver come first.
            (&[("app/B.kt", "package app\n\nfun helper() = 1\n"), ("app/A.kt", "package app\n\nclass C : Base() { fun m() { helper() } }\n")], "app/A.kt", 3, "helper", "-"),
            (&[("app/B.kt", "package app\n\nfun helper() = 1\n"), ("app/A.kt", "package app\n\nfun f() { run { helper() } }\n")], "app/A.kt", 3, "helper", "-"),
            (&[("app/B.kt", "package app\n\nfun helper() = 1\n"), ("app/A.kt", "package app\n\nfun String.g() { helper() }\n")], "app/A.kt", 3, "helper", "-"),
            (&[("app/B.kt", "package app\n\nprivate fun helper() = 1\n"), ("app/A.kt", "package app\n\nfun f() { helper() }\n")], "app/A.kt", 3, "helper", "-"),
            (&[("lib/util.dart", "int helper() => 1;\n"), ("lib/api.dart", "import 'util.dart' hide helper;\nvoid run() { helper(); }\n")], "lib/api.dart", 2, "helper", "-"),
            (&[("lib/util.dart", "int helper() => 1;\n"), ("lib/api.dart", "import 'util.dart' as u;\nvoid run() { helper(); }\n")], "lib/api.dart", 2, "helper", "-"),
            (&[("lib/util.dart", "int helper() => 1;\n"), ("lib/api.dart", "import 'util.dart';\npart 'api.g.dart';\nvoid run() { helper(); }\n")], "lib/api.dart", 3, "helper", "-"),
            (&[("packages/a/lib/util.dart", "int other() => 1;\n"), ("packages/b/lib/util.dart", "int helper() => 2;\n"), ("app/lib/api.dart", "import 'package:a/util.dart';\nvoid run() { helper(); }\n")], "app/lib/api.dart", 2, "helper", "-"),
            (&[("src/mylib/util.dart", "int helper() => 1;\n"), ("lib/api.dart", "import 'package:app/util.dart';\nvoid run() { helper(); }\n")], "lib/api.dart", 2, "helper", "-"),
            (&[("lib/helpers.rb", "def helper(x)\n  x\nend\n"), ("lib/run.rb", "require_relative 'helpers'\nclass Runner\n  def run\n    helper(1)\n  end\nend\n")], "lib/run.rb", 4, "helper", "-"),
            (&[("lib/helpers.rb", "def helper(x)\n  x\nend\n"), ("lib/other.rb", "def helper(x)\n  x\nend\n"), ("lib/run.rb", "require_relative 'helpers'\nhelper(1)\n")], "lib/run.rb", 2, "helper", "-"),
            (&[("app/store.lua", "local M = {}\nfunction M.save(x) return x end\nreturn M\n"), ("use.lua", "local store = require(\"app.store\")\nlocal function f()\n  local store = {}\n  store.save(2)\nend\n")], "use.lua", 4, "save", "-"),
            (&[("s/helpers.ps1", "function Get-Thing { param($x) $x }\n"), ("s/main.ps1", "Get-Thing -x 1\n")], "s/main.ps1", 1, "Get-Thing", "-"),
            (&[("Sources/App/Util.swift", "func helper() -> Int { 1 }\n"), ("Sources/App/Main.swift", "struct S {\n  func m() { _ = helper() }\n}\n")], "Sources/App/Main.swift", 2, "helper", "-"),
            (&[("Sources/App/Util.swift", "private func helper() -> Int { 1 }\n"), ("Sources/App/Main.swift", "func run() { _ = helper() }\n")], "Sources/App/Main.swift", 1, "helper", "-"),
            (&[("Sources/Lib/Util.swift", "func helper() -> Int { 1 }\n"), ("Sources/App/Main.swift", "func run() { _ = helper() }\n")], "Sources/App/Main.swift", 1, "helper", "-"),
            // C/C++: internal linkage, a definition behind no shared header, a member
            // context, or a definition in another namespace.
            (&[("util.h", "int add(int a, int b);\n"), ("util.c", "#include \"util.h\"\nstatic int add(int a, int b) { return a + b; }\n"), ("main.c", "#include \"util.h\"\nint main(void) { return add(1, 2); }\n")], "main.c", 2, "add", "-"),
            (&[("util.h", "int add(int a, int b);\n"), ("other.c", "int add(int a, int b) { return a - b; }\n"), ("main.c", "#include \"util.h\"\nint main(void) { return add(1, 2); }\n")], "main.c", 2, "add", "-"),
            (&[("math.h", "int add(int a, int b);\nclass Foo { void bar(); int add(int a, int b); };\n"), ("math.cpp", "#include \"math.h\"\nint add(int a, int b) { return a + b; }\n"), ("foo.cpp", "#include \"math.h\"\nvoid Foo::bar() { add(1, 2); }\n")], "foo.cpp", 2, "add", "-"),
            (&[("math.h", "namespace m { int add(int a, int b); }\nint add(int a, int b);\n"), ("math.cpp", "#include \"math.h\"\nnamespace m { int add(int a, int b) { return a + b; } }\n"), ("main.cpp", "#include \"math.h\"\nint run() { return add(1, 2); }\n")], "main.cpp", 2, "add", "-"),
        ]);
    }
}
