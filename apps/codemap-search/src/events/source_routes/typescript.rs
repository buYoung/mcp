//! ESM source bindings and byte-aligned type-only grammar recovery.
use super::syntax::*;
use std::collections::{BTreeMap, BTreeSet};

fn blank(bytes: &mut [u8], start: usize, end: usize) {
    for byte in &mut bytes[start..end] {
        if !matches!(*byte, b'\n' | b'\r') {
            *byte = b' ';
        }
    }
}
fn code_only(data: &[u8]) -> Vec<u8> {
    let mut output = data.to_vec();
    let mut index = 0;
    while index < data.len() {
        let start = index;
        if data[index..].starts_with(b"/*") {
            index = data[index + 2..]
                .windows(2)
                .position(|b| b == b"*/")
                .map(|n| index + 2 + n + 2)
                .unwrap_or(data.len());
        } else if data[index..].starts_with(b"//") {
            index = data[index + 2..]
                .iter()
                .position(|b| *b == b'\n')
                .map(|n| index + 2 + n)
                .unwrap_or(data.len());
        } else if matches!(data[index], b'\'' | b'"' | b'`') {
            let quote = data[index];
            index += 1;
            while index < data.len() {
                if data[index] == b'\\' {
                    index = (index + 2).min(data.len());
                    continue;
                }
                if data[index] == quote {
                    index += 1;
                    break;
                }
                index += 1;
            }
        } else {
            index += 1;
            continue;
        }
        blank(&mut output, start, index);
    }
    output
}
pub(crate) fn project_types(data: &str, bodies: bool) -> String {
    let code = code_only(data.as_bytes());
    let mut output = data.as_bytes().to_vec();
    let pattern = regex::bytes::Regex::new(
        r"([<,]\s*)((?:in\s+out|out|in|const)\s+)([A-Za-z_$][\w$]*)(\s*[,>:=]|\s+extends\b)",
    )
    .unwrap();
    let mut scan = code.clone();
    for _ in 0..8 {
        let ranges: Vec<_> = pattern
            .captures_iter(&scan)
            .map(|c| {
                let m = c.get(2).unwrap();
                (m.start(), m.end())
            })
            .collect();
        if ranges.is_empty() {
            break;
        }
        for (start, end) in ranges {
            blank(&mut output, start, end);
            blank(&mut scan, start, end);
        }
    }
    if bodies {
        let declaration = r"(?m)^[\t ]*(?:export[\t ]+)?(?:declare[\t ]+)?";
        let mut starts = Vec::new();
        for suffix in [
            r"(?:const|let|var)\s+[A-Za-z_$][\w$]*\s*:\s*\{",
            r"type\s+[A-Za-z_$][\w$]*\s*=\s*\{",
        ] {
            for found in regex::bytes::Regex::new(&format!("{declaration}{suffix}"))
                .unwrap()
                .find_iter(&code)
            {
                starts.push(found.end() - 1);
            }
        }
        for found in
            regex::bytes::Regex::new(&format!(r"{declaration}interface\s+[A-Za-z_$][\w$]*"))
                .unwrap()
                .find_iter(&code)
        {
            let mut depth = 0usize;
            let mut index = found.end();
            while index < code.len() {
                let byte = code[index];
                if byte == b'<' {
                    depth += 1;
                } else if byte == b'>' && code[index.saturating_sub(1)] != b'=' {
                    depth = depth.saturating_sub(1);
                } else if byte == b'{' && depth == 0 {
                    starts.push(index);
                    break;
                } else if matches!(byte, b';' | b'=') && depth == 0 {
                    break;
                }
                index += 1;
            }
        }
        for start in starts {
            let mut depth = 1usize;
            let mut index = start + 1;
            while index < code.len() && depth > 0 {
                if code[index] == b'{' {
                    depth += 1;
                } else if code[index] == b'}' {
                    depth -= 1;
                }
                index += 1;
            }
            if depth == 0 {
                blank(&mut output, start + 1, index - 1);
            }
        }
    }
    String::from_utf8(output).expect("byte-aligned ASCII type masking preserves UTF-8")
}
impl Program {
    pub fn resolve_esm(&mut self) {
        let mut definitions = BTreeMap::new();
        let mut exports: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        let mut imports: Vec<(String, String, String, String)> = Vec::new();
        let mut forwards: Vec<(String, String, String, String, bool)> = Vec::new();
        for source in self
            .sources
            .iter()
            .filter(|s| matches!(s.language.as_str(), "typescript" | "javascript"))
        {
            for &id in &source.nodes[0].children {
                let node = &source.nodes[id];
                let declaration = if node.kind == "export_statement" {
                    source.child(id, &["declaration"])
                } else {
                    Some(id)
                };
                let mut names = Vec::new();
                if let Some(declaration) = declaration {
                    match source.nodes[declaration].kind.as_str() {
                        "function_declaration"
                        | "class_declaration"
                        | "interface_declaration"
                        | "type_alias_declaration" => {
                            names.push(source.text(source.child(declaration, &["name"])).to_owned())
                        }
                        "lexical_declaration" | "variable_declaration" => {
                            for &part in &source.nodes[declaration].children {
                                if source.nodes[part].kind == "variable_declarator" {
                                    if let Some(name) = source
                                        .child(part, &["name"])
                                        .filter(|n| source.nodes[*n].kind == "identifier")
                                    {
                                        names.push(source.text(Some(name)).into());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                for name in names {
                    let value = format!("{}#{name}", source.path);
                    definitions.insert((source.path.clone(), name.clone()), value.clone());
                    if node.kind == "export_statement" {
                        exports
                            .entry((
                                source.path.clone(),
                                if node.tokens.contains("default") {
                                    "default".into()
                                } else {
                                    name
                                },
                            ))
                            .or_default()
                            .insert(value);
                    }
                }
                if !matches!(node.kind.as_str(), "export_statement" | "import_statement") {
                    continue;
                }
                let specifier = source
                    .text(source.child(id, &["source"]))
                    .trim_matches(['\'', '"']);
                let target = if specifier.is_empty() {
                    Some(source.path.clone())
                } else {
                    self.resolve_relative(&source.path, specifier)
                };
                let Some(target) = target else {
                    continue;
                };
                if node.kind == "import_statement" {
                    for part in source.walk(id, false) {
                        match source.nodes[part].kind.as_str() {
                            "import_specifier" => {
                                let name = source.text(source.child(part, &["name"]));
                                let alias = source.text(source.child(part, &["alias"]));
                                imports.push((
                                    source.path.clone(),
                                    if alias.is_empty() {
                                        name.into()
                                    } else {
                                        alias.into()
                                    },
                                    target.clone(),
                                    name.into(),
                                ));
                            }
                            "namespace_import" => {
                                let name = source.text(source.nodes[part].children.last().copied());
                                self.imports
                                    .insert((source.path.clone(), name.into()), target.clone());
                            }
                            "import_clause" => {
                                for &value in &source.nodes[part].children {
                                    if source.nodes[value].kind == "identifier" {
                                        imports.push((
                                            source.path.clone(),
                                            source.text(Some(value)).into(),
                                            target.clone(),
                                            "default".into(),
                                        ));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    let mut specific = false;
                    for part in source.walk(id, false) {
                        if source.nodes[part].kind == "export_specifier" {
                            let name = source.text(source.child(part, &["name"]));
                            let alias = source.text(source.child(part, &["alias"]));
                            forwards.push((
                                source.path.clone(),
                                if alias.is_empty() {
                                    name.into()
                                } else {
                                    alias.into()
                                },
                                target.clone(),
                                name.into(),
                                !specifier.is_empty(),
                            ));
                            specific = true;
                        } else if source.nodes[part].kind == "namespace_export" {
                            let name = source.text(source.nodes[part].children.last().copied());
                            exports
                                .entry((source.path.clone(), name.into()))
                                .or_default()
                                .insert(target.clone());
                            specific = true;
                        }
                    }
                    if !specifier.is_empty() && !specific && node.tokens.contains("*") {
                        forwards.push((source.path.clone(), "*".into(), target, "*".into(), true));
                    }
                }
            }
        }
        for _ in 0..12 {
            let before: usize = exports.values().map(BTreeSet::len).sum();
            for (path, alias, target, name) in &imports {
                if let Some(values) = exports
                    .get(&(target.clone(), name.clone()))
                    .filter(|v| v.len() == 1)
                {
                    self.imports.insert(
                        (path.clone(), alias.clone()),
                        values.first().unwrap().clone(),
                    );
                }
            }
            for (path, alias, target, name, remote) in &forwards {
                if name == "*" {
                    for ((owner, member), values) in exports.clone() {
                        if &owner == target && member != "default" {
                            exports
                                .entry((path.clone(), member))
                                .or_default()
                                .extend(values);
                        }
                    }
                } else if *remote {
                    let values = exports
                        .get(&(target.clone(), name.clone()))
                        .cloned()
                        .unwrap_or_default();
                    exports
                        .entry((path.clone(), alias.clone()))
                        .or_default()
                        .extend(values);
                } else if let Some(value) = definitions
                    .get(&(path.clone(), name.clone()))
                    .or_else(|| self.imports.get(&(path.clone(), name.clone())))
                {
                    exports
                        .entry((path.clone(), alias.clone()))
                        .or_default()
                        .insert(value.clone());
                }
            }
            if before == exports.values().map(BTreeSet::len).sum::<usize>() {
                break;
            }
        }
        self.exports = exports
            .into_iter()
            .filter(|(_, v)| v.len() == 1)
            .map(|(k, v)| (k, v.into_iter().next().unwrap()))
            .collect();
        for (path, alias, target, name) in imports {
            if let Some(value) = self.exports.get(&(target, name)) {
                self.imports.insert((path, alias), value.clone());
            } else {
                self.imports.remove(&(path, alias));
            }
        }
    }
}
