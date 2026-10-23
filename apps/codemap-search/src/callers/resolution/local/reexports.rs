//! Declarations a module file exports from other files: TypeScript/JavaScript `export ... from`
//! and `export { name }` of an imported name, the names a Python module imports, and Dart
//! `export` directives. Chains are followed a bounded number of steps, and a name that two
//! star exports both provide stays ambiguous.

use super::*;
use crate::implementations::dart_library_exports;

/// Re-export steps followed from the imported module before giving up.
const REEXPORT_DEPTH: usize = 8;

/// A declaration a module exports, possibly under another name.
pub(crate) struct ExportOrigin<'a> {
    pub(crate) file: &'a ExtractedFile,
    /// The name declared in `file`.
    pub(crate) name: String,
}

/// The origins of one exported name and the module files read to find them.
pub(in crate::callers::resolution) type ExportLookup<'a> = (Rc<Vec<ExportOrigin<'a>>>, Vec<String>);

/// One TypeScript/JavaScript export statement entry.
struct ScriptExport {
    /// The exported name; `None` for `export * from`.
    exported: Option<String>,
    /// The name in the source module or in this module's scope.
    original: String,
    /// The module of `export ... from`; `None` for a local export clause.
    module: Option<String>,
}

impl<'a> SourceResolver<'a> {
    /// Declarations the module file at `path` exports as `name`: its own top-level declaration,
    /// else what its re-exports name. Several origins mean overlapping star exports.
    pub(crate) fn export_origins(&self, path: &str, name: &str) -> Rc<Vec<ExportOrigin<'a>>> {
        let key = (path.to_string(), name.to_string());
        let cached = self
            .export_origins
            .borrow()
            .get(&key)
            .map(|(origins, files_read)| (Rc::clone(origins), files_read.clone()));
        if let Some((origins, files_read)) = cached {
            // Reading again records the module files as evidence of this resolution too.
            for file in &files_read {
                self.source(file);
            }
            return origins;
        }
        let mut files_read = Vec::new();
        let origins = Rc::new(match self.files_by_path.get(path).copied() {
            Some(module) => self.origins_at(module, name, 0, &mut HashSet::new(), &mut files_read),
            None => Vec::new(),
        });
        self.export_origins
            .borrow_mut()
            .insert(key, (Rc::clone(&origins), files_read));
        origins
    }

    fn origins_at(
        &self,
        module: &'a ExtractedFile,
        name: &str,
        depth: usize,
        visited: &mut HashSet<(String, String)>,
        files_read: &mut Vec<String>,
    ) -> Vec<ExportOrigin<'a>> {
        if depth > REEXPORT_DEPTH || !visited.insert((module.file_path.clone(), name.to_string())) {
            return Vec::new();
        }
        if module.symbols.iter().any(|symbol| {
            symbol.name == name && symbol.owner.is_none() && is_top_level(module, symbol)
        }) {
            return vec![ExportOrigin {
                file: module,
                name: name.to_string(),
            }];
        }
        let Some(language) = crate::lang::spec_for_path(Path::new(&module.file_path))
            .map(|spec| spec.language_name())
        else {
            return Vec::new();
        };
        let Some(source) = self.source(&module.file_path) else {
            return Vec::new();
        };
        if !files_read.contains(&module.file_path) {
            files_read.push(module.file_path.clone());
        }
        let imports = source
            .extracted
            .navigation
            .as_ref()
            .map(|navigation| navigation.imports.as_slice())
            .unwrap_or_default();
        let mut stars = Vec::new();
        match language {
            "typescript" | "javascript" => {
                let exports = script_exports(&source);
                // A named export shadows every star export.
                if let Some(export) = exports
                    .iter()
                    .find(|export| export.exported.as_deref() == Some(name))
                {
                    let next = match &export.module {
                        Some(specifier) => self
                            .script_module(&module.file_path, specifier)
                            .map(|file| (file, export.original.clone())),
                        // `export { original as name }` of an imported binding.
                        None => only_binding(imports, &export.original)
                            .flatten()
                            .filter(|import| import.kind == ImportKind::Named)
                            .and_then(|import| {
                                let file = self
                                    .script_module(&module.file_path, import.source.as_deref()?)?;
                                Some((file, import.imported_name.clone()?))
                            }),
                    };
                    return next
                        .map(|(file, original)| {
                            self.origins_at(file, &original, depth + 1, visited, files_read)
                        })
                        .unwrap_or_default();
                }
                stars = exports
                    .iter()
                    .filter(|export| export.exported.is_none())
                    .filter_map(|export| {
                        self.script_module(&module.file_path, export.module.as_deref()?)
                    })
                    .collect();
            }
            "python" => {
                match only_binding(imports, name) {
                    // Disagreeing imports, as in `try`/`except` fallbacks.
                    None => return Vec::new(),
                    Some(Some(import)) => {
                        let next = import
                            .source
                            .as_deref()
                            .and_then(|module_name| {
                                self.python_module(&module.file_path, module_name)
                            })
                            .zip(import.imported_name.clone());
                        return next
                            .map(|(file, original)| {
                                self.origins_at(file, &original, depth + 1, visited, files_read)
                            })
                            .unwrap_or_default();
                    }
                    Some(None) => {}
                }
                // A star import does not bind names with a leading underscore.
                if !name.starts_with('_') {
                    stars = imports
                        .iter()
                        .filter(|import| import.local_name == "*")
                        .filter_map(|import| {
                            self.python_module(&module.file_path, import.source.as_deref()?)
                        })
                        .collect();
                }
            }
            "dart" if !name.starts_with('_') => {
                for export in
                    dart_library_exports(source.syntax.tree.root_node(), source.text.as_bytes())
                {
                    if !export.shown.is_empty() && !export.shown.iter().any(|shown| shown == name)
                        || export.hidden.iter().any(|hidden| hidden == name)
                    {
                        continue;
                    }
                    stars.extend(self.dart_library_files(&module.file_path, &export.uri));
                }
            }
            _ => {}
        }
        let mut origins: Vec<ExportOrigin<'a>> = Vec::new();
        for file in stars {
            for origin in self.origins_at(file, name, depth + 1, visited, files_read) {
                if !origins
                    .iter()
                    .any(|known| std::ptr::eq(known.file, origin.file) && known.name == origin.name)
                {
                    origins.push(origin);
                }
            }
        }
        origins
    }
}

/// The export statements of a TypeScript/JavaScript module. `default` exports and
/// `export * as name` namespace objects are left out.
fn script_exports(source: &Source) -> Vec<ScriptExport> {
    let root = source.syntax.tree.root_node();
    let mut exports = Vec::new();
    let mut cursor = root.walk();
    for statement in root
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "export_statement")
    {
        let module = statement
            .child_by_field_name("source")
            .and_then(|node| text(node, source))
            .map(|specifier| specifier.trim_matches(['"', '\'', '`']).to_string());
        let mut inner = statement.walk();
        let children = statement.named_children(&mut inner).collect::<Vec<_>>();
        if let Some(clause) = children.iter().find(|node| node.kind() == "export_clause") {
            let mut specifiers = clause.walk();
            for specifier in clause
                .named_children(&mut specifiers)
                .filter(|node| node.kind() == "export_specifier")
            {
                let Some(original) = specifier
                    .child_by_field_name("name")
                    .and_then(|node| text(node, source))
                else {
                    continue;
                };
                let exported = specifier
                    .child_by_field_name("alias")
                    .and_then(|node| text(node, source))
                    .unwrap_or(original);
                if original == "default" || exported == "default" {
                    continue;
                }
                exports.push(ScriptExport {
                    exported: Some(exported.to_string()),
                    original: original.to_string(),
                    module: module.clone(),
                });
            }
        } else if module.is_some()
            && !children
                .iter()
                .any(|node| node.kind() == "namespace_export")
        {
            exports.push(ScriptExport {
                exported: None,
                original: String::new(),
                module,
            });
        }
    }
    exports
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

    const HELPER: (&str, &str) = ("src/util/helper.ts", "export function helper() {}\n");

    #[test]
    fn script_calls_follow_reexports_to_the_declaring_module() {
        check(&[
            (&[HELPER, ("src/util/index.ts", "export { helper } from './helper';\n"), ("src/main.ts", "import { helper } from './util';\nhelper();\n")], "src/main.ts", 2, "helper", "src/util/helper.ts:1"),
            (&[HELPER, ("src/util/index.ts", "export { helper as assist } from './helper';\n"), ("src/main.ts", "import { assist } from './util';\nassist();\n")], "src/main.ts", 2, "assist", "src/util/helper.ts:1"),
            (&[HELPER, ("src/util/index.ts", "import { helper } from './helper';\nexport { helper };\n"), ("src/main.ts", "import { helper } from './util';\nhelper();\n")], "src/main.ts", 2, "helper", "src/util/helper.ts:1"),
            (&[HELPER, ("src/util/nested.ts", "export * from './helper';\n"), ("src/util/index.ts", "export * from './nested';\n"), ("src/main.ts", "import { helper } from './util';\nhelper();\n")], "src/main.ts", 2, "helper", "src/util/helper.ts:1"),
            (&[HELPER, ("src/util/index.ts", "export * from './helper';\n"), ("src/main.ts", "import * as util from './util';\nutil.helper();\n")], "src/main.ts", 2, "helper", "src/util/helper.ts:1"),
            // A named export shadows a star export of the same name.
            (&[HELPER, ("src/util/other.ts", "export function helper() {}\n"), ("src/util/index.ts", "export * from './other';\nexport { helper } from './helper';\n"), ("src/main.ts", "import { helper } from './util';\nhelper();\n")], "src/main.ts", 2, "helper", "src/util/helper.ts:1"),
            // A type re-exported by a barrel types the receiver of a method call.
            (&[("src/services/user.service.ts", "export class UserService {\n  save() {}\n}\n"), ("src/services/index.ts", "export * from './user.service';\n"), ("src/api.ts", "import { UserService } from './services';\nexport function run(service: UserService) {\n  service.save();\n}\n")], "src/api.ts", 3, "save", "src/services/user.service.ts:2"),
        ]);
    }

    #[test]
    fn ambiguous_or_cyclic_script_reexports_stay_unresolved() {
        check(&[
            (
                &[
                    HELPER,
                    ("src/util/other.ts", "export function helper() {}\n"),
                    (
                        "src/util/index.ts",
                        "export * from './helper';\nexport * from './other';\n",
                    ),
                    (
                        "src/main.ts",
                        "import { helper } from './util';\nhelper();\n",
                    ),
                ],
                "src/main.ts",
                2,
                "helper",
                "-",
            ),
            (
                &[
                    ("src/a.ts", "export * from './b';\n"),
                    ("src/b.ts", "export * from './a';\n"),
                    ("src/main.ts", "import { helper } from './a';\nhelper();\n"),
                ],
                "src/main.ts",
                2,
                "helper",
                "-",
            ),
            (
                &[
                    HELPER,
                    ("src/util/index.ts", "export * as tools from './helper';\n"),
                    (
                        "src/main.ts",
                        "import { helper } from './util';\nhelper();\n",
                    ),
                ],
                "src/main.ts",
                2,
                "helper",
                "-",
            ),
        ]);
    }

    #[test]
    fn python_and_dart_calls_follow_package_and_library_exports() {
        check(&[
            (&[("pkg/helpers.py", "def url_for():\n    pass\n"), ("pkg/__init__.py", "from .helpers import url_for\n"), ("app.py", "from pkg import url_for\n\nurl_for()\n")], "app.py", 3, "url_for", "pkg/helpers.py:1"),
            (&[("pkg/helpers.py", "def url_for():\n    pass\n"), ("pkg/__init__.py", "from .helpers import *\n"), ("app.py", "import pkg\n\npkg.url_for()\n")], "app.py", 3, "url_for", "pkg/helpers.py:1"),
            (&[("pkg/app.py", "class Flask:\n    def run(self):\n        pass\n"), ("pkg/__init__.py", "from .app import Flask\n"), ("main.py", "from pkg import Flask\n\ndef go(app: Flask):\n    app.run()\n")], "main.py", 4, "run", "pkg/app.py:2"),
            (&[("lib/src/store.dart", "int helper() => 1;\nclass Store {\n  void save() {}\n}\n"), ("lib/app.dart", "export 'src/store.dart';\n"), ("lib/main.dart", "import 'app.dart';\nvoid run(Store store) {\n  helper();\n  store.save();\n}\n")], "lib/main.dart", 3, "helper", "lib/src/store.dart:1"),
            (&[("lib/src/store.dart", "int helper() => 1;\nclass Store {\n  void save() {}\n}\n"), ("lib/app.dart", "export 'src/store.dart';\n"), ("lib/main.dart", "import 'app.dart';\nvoid run(Store store) {\n  helper();\n  store.save();\n}\n")], "lib/main.dart", 4, "save", "lib/src/store.dart:3"),
            // Names a star import or a `hide` list leaves out are not exported.
            (&[("pkg/helpers.py", "def _private():\n    pass\n"), ("pkg/__init__.py", "from .helpers import *\n"), ("app.py", "from pkg import _private\n\n_private()\n")], "app.py", 3, "_private", "-"),
            (&[("lib/src/store.dart", "int helper() => 1;\n"), ("lib/app.dart", "export 'src/store.dart' hide helper;\n"), ("lib/main.dart", "import 'app.dart';\nvoid run() {\n  helper();\n}\n")], "lib/main.dart", 3, "helper", "-"),
        ]);
    }

    #[test]
    fn jsconfig_and_extended_package_configurations_map_module_paths() {
        check(&[
            (&[("jsconfig.json", "{ \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"src/*\"] } } }\n"), ("src/lib/helper.js", "export function helper() {}\n"), ("src/main.js", "import { helper } from '@/lib/helper';\nhelper();\n")], "src/main.js", 2, "helper", "src/lib/helper.js:1"),
            (&[("node_modules/@repo/tsconfig/base.json", "{ \"compilerOptions\": { \"paths\": { \"@/*\": [\"../../../src/*\"] } } }\n"), ("tsconfig.json", "{ \"extends\": \"@repo/tsconfig/base.json\" }\n"), ("src/lib/helper.ts", "export function helper() {}\n"), ("src/main.ts", "import { helper } from '@/lib/helper';\nhelper();\n")], "src/main.ts", 2, "helper", "src/lib/helper.ts:1"),
            // Of several base configurations, the last one that sets `paths` wins.
            (&[("a.json", "{ \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"src/*\"] } } }\n"), ("b.json", "{ \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"lib/*\"] } } }\n"), ("tsconfig.json", "{ \"extends\": [\"./a.json\", \"./b.json\"] }\n"), ("src/helper.ts", "export function helper() {}\n"), ("lib/helper.ts", "export function helper() {}\n"), ("src/main.ts", "import { helper } from '@/helper';\nhelper();\n")], "src/main.ts", 2, "helper", "lib/helper.ts:1"),
        ]);
    }
}
