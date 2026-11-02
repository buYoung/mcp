//! How an import of one file reaches a declaration of another: the reverse of call resolution,
//! for listing who imports a declaration. Each language's own rule decides, through the module,
//! header, package and namespace lookups that call resolution already uses.

use super::*;
use crate::implementations::relative_path;
use crate::parser::ImportEntry;
use module_functions::{package_of, swift_module};

/// How one import makes a declaration of another file, or that file as a whole, usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImportReach {
    /// Bound to this local name. Empty when the binding is not recorded, as for a Nix import.
    Bound(String),
    /// Usable under its own name: an include, a sourced script, a library, package, glob or
    /// namespace import.
    Bare,
    /// Usable as this prefix followed by its name, as `config.Name` or `config::Name`.
    Qualified(String),
}

impl<'a> SourceResolver<'a> {
    /// How `import` of `importer` reaches `declaration` of `target`, or `None`. Without a
    /// declaration it asks about the file as a whole: what a default import, a component
    /// import, a Nix `import` or a Lua `require` binds.
    pub(crate) fn import_reach(
        &self,
        importer: &'a ExtractedFile,
        import: &ImportEntry,
        target: &'a ExtractedFile,
        declaration: Option<&ExtractedSymbol>,
    ) -> Option<ImportReach> {
        let language = crate::lang::spec_for_path(Path::new(&importer.file_path))?.language_name();
        let target_language =
            crate::lang::spec_for_path(Path::new(&target.file_path))?.language_name();
        if import_family(language) != import_family(target_language) {
            return None;
        }
        let local = || import.local_name.clone();
        let imported = import
            .imported_name
            .as_deref()
            .unwrap_or(&import.local_name);
        let is_target = |file: &ExtractedFile| file.file_path == target.file_path;
        let source = import.source.as_deref();
        match (language, declaration) {
            ("typescript" | "javascript" | "vue" | "astro" | "svelte", declaration) => {
                let module = self.script_module(&importer.file_path, source?)?;
                // The module another one passes on as `imported`, by `export * as imported from`
                // or `export { default as imported } from`.
                let passed_on = |is_namespace: bool| {
                    module
                        .navigation
                        .iter()
                        .flat_map(|navigation| &navigation.renamed_exports)
                        .filter(|export| export.local_name == imported)
                        .filter(|export| {
                            if is_namespace {
                                export.kind == ImportKind::Namespace
                            } else {
                                export.imported_name.as_deref() == Some("default")
                            }
                        })
                        .find_map(|export| {
                            self.script_module(&module.file_path, export.source.as_deref()?)
                        })
                };
                match (&import.kind, declaration) {
                    (ImportKind::Named, Some(declaration)) if imported != "default" => {
                        if self.exports(module, imported, target, &declaration.name) {
                            return Some(ImportReach::Bound(local()));
                        }
                        let namespace = passed_on(true)?;
                        self.exports(namespace, &declaration.name, target, &declaration.name)
                            .then(|| ImportReach::Qualified(format!("{}.", import.local_name)))
                    }
                    (ImportKind::Namespace, Some(declaration)) => self
                        .exports(module, &declaration.name, target, &declaration.name)
                        .then(|| ImportReach::Qualified(format!("{}.", import.local_name))),
                    (ImportKind::Default, None) | (ImportKind::Named, None)
                        if import.kind == ImportKind::Default || imported == "default" =>
                    {
                        is_target(module).then(|| ImportReach::Bound(local()))
                    }
                    (ImportKind::Named, None) => passed_on(false)
                        .is_some_and(is_target)
                        .then(|| ImportReach::Bound(local())),
                    _ => None,
                }
            }
            ("python", Some(declaration)) => match &import.kind {
                ImportKind::Named => {
                    let source = source?;
                    if let Some(module) = self.python_module(&importer.file_path, source) {
                        if self.exports(module, imported, target, &declaration.name) {
                            return Some(ImportReach::Bound(local()));
                        }
                    }
                    // `from package import module` and `import package.module` bind a module.
                    let separator = if source.ends_with('.') { "" } else { "." };
                    let submodule = format!("{source}{separator}{imported}");
                    is_target(self.python_module(&importer.file_path, &submodule)?)
                        .then(|| ImportReach::Qualified(format!("{}.", import.local_name)))
                }
                ImportKind::Default => {
                    let module =
                        self.python_module(&importer.file_path, source.unwrap_or(imported))?;
                    is_target(module)
                        .then(|| ImportReach::Qualified(format!("{}.", import.local_name)))
                }
                _ => None,
            },
            ("dart", Some(_)) => self
                .dart_library_files(&importer.file_path, source?)
                .iter()
                .any(|file| is_target(file))
                .then_some(ImportReach::Bare),
            ("lua", None) => {
                let module = self
                    .relative_imports(&importer.file_path, std::slice::from_ref(import), &["lua"])
                    .into_iter()
                    .next()
                    .or_else(|| self.lua_module(source?))?;
                is_target(module).then(|| ImportReach::Bound(local()))
            }
            ("ruby" | "powershell", Some(_)) => {
                let extensions: &[&str] = if language == "ruby" {
                    &["rb"]
                } else {
                    &["ps1", "psm1"]
                };
                let is_sourced = language == "powershell"
                    && self
                        .included_file(&importer.file_path, source?)
                        .is_some_and(is_target);
                (is_sourced
                    || self
                        .relative_imports(
                            &importer.file_path,
                            std::slice::from_ref(import),
                            extensions,
                        )
                        .iter()
                        .any(|file| is_target(file)))
                .then_some(ImportReach::Bare)
            }
            ("c" | "cpp" | "asm" | "bash" | "zsh", Some(_)) => self
                .included_file(&importer.file_path, source?)
                .is_some_and(is_target)
                .then_some(ImportReach::Bare),
            ("nix", None) => self
                .included_file(&importer.file_path, source?)
                .is_some_and(is_target)
                .then(|| ImportReach::Bound(String::new())),
            ("starlark", Some(_)) => self
                .starlark_file(&importer.file_path, source?)
                .is_some_and(is_target)
                .then_some(ImportReach::Bare),
            ("php", Some(declaration)) => {
                let source = source?;
                if source.contains('\\') {
                    let namespace = namespace_of(target, declaration)?;
                    let source = source.trim_start_matches('\\').replace('\\', ".");
                    (source == format!("{namespace}.{}", declaration.name))
                        .then(|| ImportReach::Bound(local()))
                } else {
                    self.included_file(&importer.file_path, source)
                        .is_some_and(is_target)
                        .then_some(ImportReach::Bare)
                }
            }
            ("go", Some(_)) => {
                let directory = self.go_import_directory(&importer.file_path, source?)?;
                if Some(directory.as_path()) != self.root.join(&target.file_path).parent() {
                    return None;
                }
                if import.kind == ImportKind::Glob {
                    return Some(ImportReach::Bare);
                }
                // Without an alias the package clause names the binding.
                let binding = if import.local_name == imported {
                    self.go_package(&target.file_path)?
                } else {
                    local()
                };
                Some(ImportReach::Qualified(format!("{binding}.")))
            }
            ("java" | "kotlin", Some(declaration)) => {
                let package = package_of(target)?;
                (import.kind == ImportKind::Named
                    && source == Some(package.as_str())
                    && imported == declaration.name)
                    .then(|| ImportReach::Bound(local()))
            }
            ("scala" | "groovy", Some(declaration)) => {
                let package = package_of(target)?;
                let source = source?;
                if source == format!("{package}.{}", declaration.name) {
                    Some(ImportReach::Bound(local()))
                } else {
                    [
                        package.clone(),
                        format!("{package}._"),
                        format!("{package}.*"),
                    ]
                    .contains(&source.to_string())
                    .then_some(ImportReach::Bare)
                }
            }
            ("csharp", Some(declaration)) => {
                let namespace = namespace_of(target, declaration)?;
                let source = source?;
                if source == namespace {
                    Some(ImportReach::Bare)
                } else {
                    (source == format!("{namespace}.{}", declaration.name))
                        .then(|| ImportReach::Bound(local()))
                }
            }
            ("swift", Some(_)) => {
                let module = swift_module(&target.file_path);
                let name = module.rsplit('/').next().unwrap_or(&module);
                (source == Some(name)).then_some(ImportReach::Bare)
            }
            ("rust", Some(declaration)) => {
                let kind = match declaration.kind.as_str() {
                    "const" | "static" => "const",
                    "fn" => "call",
                    _ => "type",
                };
                let source = source?;
                let reaches = |path: &str| {
                    self.lookup_at_root(importer, path, kind, 0)
                        .is_some_and(|found| {
                            is_target(found.file) && found.symbol.range == declaration.range
                        })
                };
                let member = format!("{source}::{}", declaration.name);
                if import.kind == ImportKind::Glob {
                    reaches(&member).then_some(ImportReach::Bare)
                } else if imported == declaration.name && reaches(source) {
                    Some(ImportReach::Bound(local()))
                } else {
                    reaches(&member)
                        .then(|| ImportReach::Qualified(format!("{}::", import.local_name)))
                }
            }
            _ => None,
        }
    }

    /// Positions in the indexed files of the other files that see `target`'s declarations
    /// without importing them: the same Go package, JVM package, C# or PHP namespace, or Swift
    /// module.
    pub(crate) fn package_peers(&self, target: &'a ExtractedFile) -> Vec<usize> {
        let language_of = |file: &ExtractedFile| {
            crate::lang::spec_for_path(Path::new(&file.file_path)).map(|spec| spec.language_name())
        };
        let Some(language) = language_of(target) else {
            return Vec::new();
        };
        let family = import_family;
        let is_peer: Box<dyn Fn(&ExtractedFile) -> bool> = match language {
            // Go keeps one package per directory; test packages are left out with test files.
            "go" => {
                let directory = Path::new(&target.file_path).parent();
                Box::new(move |file| Path::new(&file.file_path).parent() == directory)
            }
            "java" | "kotlin" | "scala" | "groovy" | "csharp" | "php" => {
                let packages = namespaces(target);
                if packages.is_empty() {
                    return Vec::new();
                }
                Box::new(move |file| {
                    namespaces(file)
                        .iter()
                        .any(|namespace| packages.contains(namespace))
                })
            }
            "swift" => {
                let module = swift_module(&target.file_path);
                Box::new(move |file| swift_module(&file.file_path) == module)
            }
            _ => return Vec::new(),
        };
        self.files
            .iter()
            .enumerate()
            .filter(|(_, file)| {
                file.file_path != target.file_path
                    && is_peer(file)
                    && language_of(file).is_some_and(|other| family(other) == family(language))
            })
            .map(|(position, _)| position)
            .collect()
    }

    /// Whether `module` exports `exported` as declaration `name` of `target`, itself or
    /// through its re-exports.
    fn exports(
        &self,
        module: &'a ExtractedFile,
        exported: &str,
        target: &ExtractedFile,
        name: &str,
    ) -> bool {
        self.export_origins(&module.file_path, exported)
            .iter()
            .any(|origin| origin.file.file_path == target.file_path && origin.name == name)
    }

    /// The indexed file a header include, sourced script, Nix `import` or `.include` path
    /// names: beside the including file, or the one indexed file ending with that path. A
    /// computed leading part, as a script's own directory, leaves its literal rest to name it.
    fn included_file(&self, importer: &str, include: &str) -> Option<&'a ExtractedFile> {
        let include = crate::lang::literal_path_tail(include)?.replace('\\', "/");
        let include = include.trim_start_matches("./");
        if let Some(beside) =
            relative_path(importer, include).and_then(|path| self.indexed_file(&path))
        {
            return Some(beside);
        }
        let lookups = self.module_lookups();
        let name = include.rsplit('/').next().unwrap_or(include);
        let suffix = format!("/{include}");
        let matches: Vec<_> = lookups
            .by_file_name
            .get(name)
            .into_iter()
            .flatten()
            .filter(|file| file.file_path == include || file.file_path.ends_with(&suffix))
            .collect();
        match matches.as_slice() {
            [only] => Some(**only),
            _ => None,
        }
    }

    /// The indexed file at `path`, in any language.
    fn indexed_file(&self, path: &str) -> Option<&'a ExtractedFile> {
        let name = path.rsplit('/').next()?;
        self.module_lookups()
            .by_file_name
            .get(name)?
            .iter()
            .copied()
            .find(|file| file.file_path == path)
    }

    /// The indexed file a Starlark `load` label names: `//package:file.bzl` from the workspace
    /// root, `:file.bzl` from the loading file's package. Labels of other repositories name no
    /// indexed file.
    fn starlark_file(&self, importer: &str, label: &str) -> Option<&'a ExtractedFile> {
        let path = if let Some(rest) = label.strip_prefix("//") {
            rest.replacen(':', "/", 1)
        } else {
            let file = label.strip_prefix(':')?;
            match Path::new(importer)
                .parent()
                .and_then(|directory| directory.to_str())
            {
                Some(directory) if !directory.is_empty() => format!("{directory}/{file}"),
                _ => file.to_string(),
            }
        };
        self.indexed_file(&path)
    }
}

/// Whether an import in `language` can reach another file without naming one of its
/// declarations, so that its module path decides: a whole-file, namespace or side-effect import
/// of a script module, a module, package, header or script import elsewhere. Script and
/// Java/Kotlin named imports reach only the declaration they name.
pub(crate) fn may_reach_unnamed(language: &str, import: &ImportEntry) -> bool {
    match language {
        "typescript" | "javascript" | "vue" | "astro" | "svelte" | "java" | "kotlin" => {
            import.kind != ImportKind::Named
        }
        _ => true,
    }
}

/// Languages whose files import one another: script modules and their components, the JVM
/// languages, C-family headers and assembly includes, and the two shells.
fn import_family(language: &str) -> &str {
    match language {
        "typescript" | "javascript" | "vue" | "astro" | "svelte" => "script",
        "java" | "kotlin" | "scala" | "groovy" => "jvm",
        "c" | "cpp" | "asm" => "c",
        "bash" | "zsh" => "shell",
        other => other,
    }
}

/// The package or namespace holding `declaration`: the file's package or file-scoped
/// namespace, else the innermost namespace block around it, with `\` written as `.`.
fn namespace_of(file: &ExtractedFile, declaration: &ExtractedSymbol) -> Option<String> {
    package_of(file).or_else(|| {
        file.symbols
            .iter()
            .filter(|symbol| {
                symbol.kind == "mod"
                    && symbol.range.start_line <= declaration.range.start_line
                    && declaration.range.end_line_inclusive() <= symbol.range.end_line_inclusive()
            })
            .min_by_key(|symbol| symbol.range.end_line_inclusive() - symbol.range.start_line)
            .map(|symbol| symbol.name.replace('\\', "."))
    })
}

/// Every package or namespace a file declares, with `\` written as `.`.
fn namespaces(file: &ExtractedFile) -> Vec<String> {
    package_of(file)
        .into_iter()
        .chain(
            file.symbols
                .iter()
                .filter(|symbol| symbol.kind == "mod")
                .map(|symbol| symbol.name.replace('\\', ".")),
        )
        .collect()
}
