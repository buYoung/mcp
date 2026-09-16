//! Address provenance for the tested RGBDS/x86 subset, without emulation.
use super::model::*;
use super::syntax::Source;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone)]
struct Address {
    table: Value,
    index: Value,
    is_loaded: bool,
}
fn register(operand: &str) -> Option<(String, usize)> {
    let operand = operand.trim().to_ascii_lowercase();
    for (prefix, short) in [
        ("a", "ax"),
        ("b", "bx"),
        ("c", "cx"),
        ("d", "dx"),
        ("si", "si"),
        ("di", "di"),
        ("sp", "sp"),
        ("bp", "bp"),
    ] {
        let mut aliases = vec![
            (format!("r{short}"), 64),
            (format!("e{short}"), 32),
            (short.into(), 16),
        ];
        aliases.push((format!("{prefix}l"), 8));
        if prefix.len() == 1 {
            aliases.push((format!("{prefix}h"), 8));
        }
        if let Some((_, width)) = aliases.iter().find(|(name, _)| name == &operand) {
            return Some((format!("r{short}"), *width));
        }
    }
    let rest = operand.strip_prefix('r')?;
    let (digits, width) = if let Some(digits) = rest.strip_suffix('d') {
        (digits, 32)
    } else if let Some(digits) = rest.strip_suffix('w') {
        (digits, 16)
    } else if let Some(digits) = rest.strip_suffix('b') {
        (digits, 8)
    } else {
        (rest, 64)
    };
    let number = digits.parse::<u8>().ok()?;
    (8..=15)
        .contains(&number)
        .then(|| (format!("r{number}"), width))
}
pub(super) fn analyze(sources: &[Arc<Source>]) -> (Vec<Fact>, usize) {
    let sources: Vec<_> = sources
        .iter()
        .filter(|s| s.language == "assembly")
        .collect();
    let label_re = regex::Regex::new(r"^([A-Za-z_.$][\w.$]*):{1,2}(.*)$").unwrap();
    let directive = regex::Regex::new(r"(?i)^(?:dw|dd|dq|\.word|\.long|\.quad)\s+(.+)$").unwrap();
    let symbol = regex::Regex::new(r"^[A-Za-z_.$][\w.$]*$").unwrap();
    let mut tables = BTreeMap::new();
    let mut facts = Vec::new();
    let mut omitted = 0;
    for source in &sources {
        let mut label = String::new();
        let mut index = 0;
        let mut declaration = None;
        for (number, line) in source.data.lines().enumerate() {
            let mut code = line.split(';').next().unwrap_or_default().trim().to_owned();
            if let Some(capture) = label_re.captures(&code) {
                label = capture[1].into();
                code = capture[2].trim().into();
                index = 0;
                declaration = Some(Location {
                    path: source.path.clone(),
                    line: number + 1,
                    column: 1,
                });
            }
            let Some(capture) = directive.captures(&code).filter(|_| !label.is_empty()) else {
                continue;
            };
            for value in capture[1].split(',').map(str::trim) {
                if symbol.is_match(value) {
                    if facts.len() >= super::engine::TOTAL_FACTS {
                        omitted += 1;
                        index += 1;
                        continue;
                    }
                    let root = Value::new("global", format!("{}::{label}", source.path));
                    tables.insert((source.path.clone(), label.clone()), root.clone());
                    facts.push(Fact::new(
                        "store",
                        root.slot(Value::entries())
                            .slot(Value::new("literal", index.to_string())),
                        Value::new("function", format!("{}::{value}", source.path)),
                        Location {
                            path: source.path.clone(),
                            line: number + 1,
                            column: 1,
                        },
                        &format!("table:{label}"),
                        vec![
                            "assembly_address_table".into(),
                            "linker_binding_unproven".into(),
                        ],
                    ));
                }
                index += 1;
                if let (Some(fact), Some(declaration)) = (
                    facts.last_mut().filter(|f| {
                        f.location.path == source.path && f.location.line == number + 1
                    }),
                    declaration.as_ref(),
                ) {
                    if &fact.location != declaration && !fact.via.contains(declaration) {
                        fact.via.push(declaration.clone());
                    }
                }
            }
        }
    }
    let mode = regex::Regex::new(r"(?i)^bits\s+(32|64)$").unwrap();
    let instruction = regex::Regex::new(r"^(\w+)\s*(.*)").unwrap();
    let qualifier = regex::Regex::new(r"(?i)\b(?:rel|qword|dword|ptr)\b|[\[\]]").unwrap();
    for source in sources {
        let mut registers: BTreeMap<String, Address> = BTreeMap::new();
        let mut byte_sources: BTreeMap<String, (Value, &str)> = BTreeMap::new();
        let mut hl_address = None;
        let mut pointer_bits = 64;
        let mut declaration = None;
        for (number, line) in source.data.lines().enumerate() {
            let code = line.split(';').next().unwrap_or_default().trim();
            if code.is_empty() {
                continue;
            }
            if label_re.is_match(code) {
                registers.clear();
                byte_sources.clear();
                hl_address = None;
                declaration = Some(Location {
                    path: source.path.clone(),
                    line: number + 1,
                    column: 1,
                });
                continue;
            }
            if let Some(bits) = mode.captures(code) {
                pointer_bits = bits[1].parse().unwrap();
                registers.clear();
                continue;
            }
            let Some(parts) = instruction.captures(code) else {
                continue;
            };
            let operation = parts[1].to_ascii_lowercase();
            let operands: Vec<_> = parts[2].split(',').map(str::trim).collect();
            let destination = operands[0].to_ascii_lowercase();
            if matches!(operation.as_str(), "ld" | "lea" | "mov") && operands.len() == 2 {
                let raw = operands[1];
                let operand = qualifier.replace_all(raw, " ").trim().to_owned();
                let candidates: Vec<_> = tables
                    .iter()
                    .filter(|((_, name), _)| name == &operand)
                    .map(|(_, v)| v.clone())
                    .collect();
                let table = tables
                    .get(&(source.path.clone(), operand.clone()))
                    .cloned()
                    .or_else(|| (candidates.len() == 1).then(|| candidates[0].clone()));
                if operation == "ld" {
                    if destination == "hl" {
                        hl_address = if raw.contains('[') { None } else { table };
                        byte_sources.remove("h");
                        byte_sources.remove("l");
                    } else if matches!(destination.as_str(), "a" | "h" | "l") {
                        if destination == "a"
                            && raw.eq_ignore_ascii_case("[hli]")
                            && hl_address.is_some()
                        {
                            byte_sources.insert("a".into(), (hl_address.clone().unwrap(), "low"));
                        } else if destination == "h"
                            && raw.eq_ignore_ascii_case("[hl]")
                            && hl_address.is_some()
                        {
                            byte_sources.insert("h".into(), (hl_address.clone().unwrap(), "high"));
                        } else if let Some(value) =
                            byte_sources.get(&raw.to_ascii_lowercase()).cloned()
                        {
                            byte_sources.insert(destination.clone(), value);
                        } else {
                            byte_sources.remove(&destination);
                        }
                        if matches!(destination.as_str(), "h" | "l") {
                            hl_address = None;
                        }
                    }
                    continue;
                }
                if let Some((canonical, width)) = register(&destination) {
                    let prior = registers.clone();
                    registers.remove(&canonical);
                    if width == pointer_bits {
                        if let Some(table) = table {
                            registers.insert(
                                canonical,
                                Address {
                                    table,
                                    index: Value::new("literal", "0"),
                                    is_loaded: operation == "mov" && raw.contains('['),
                                },
                            );
                        } else if let Some((input, input_width)) = register(&operand) {
                            if input_width == pointer_bits {
                                if let Some(origin) = prior.get(&input) {
                                    if !raw.contains('[') && operation == "mov" {
                                        registers.insert(canonical, origin.clone());
                                    } else if raw.contains('[')
                                        && operation == "mov"
                                        && !origin.is_loaded
                                    {
                                        let mut loaded = origin.clone();
                                        loaded.is_loaded = true;
                                        registers.insert(canonical, loaded);
                                    }
                                }
                            }
                        }
                    }
                }
                continue;
            }
            if matches!(operation.as_str(), "jp" | "jmp" | "call") {
                let mut address = None;
                if destination == "hl" {
                    if let (Some(low), Some(high)) = (byte_sources.get("l"), byte_sources.get("h"))
                    {
                        if low.0 == high.0 && low.1 == "low" && high.1 == "high" {
                            address = Some(Address {
                                table: low.0.clone(),
                                index: Value::new(
                                    "dynamic_key",
                                    format!("{}:index:{}", source.path, number + 1),
                                ),
                                is_loaded: true,
                            });
                        }
                    }
                }
                if let Some((name, width)) = register(destination.trim_start_matches('*')) {
                    if width == pointer_bits {
                        address = registers.get(&name).cloned();
                    }
                }
                if let Some(address) = address.filter(|a| a.is_loaded) {
                    if facts.len() >= super::engine::TOTAL_FACTS {
                        omitted += 1;
                        registers.clear();
                        byte_sources.clear();
                        hl_address = None;
                        continue;
                    }
                    facts.push(Fact::new(
                        "invoke",
                        address.table.slot(Value::entries()).slot(address.index),
                        Value::unknown(),
                        Location {
                            path: source.path.clone(),
                            line: number + 1,
                            column: 1,
                        },
                        &format!("indirect:{}", source.path),
                        vec![
                            "assembly_control_flow_unproven".into(),
                            "table_entry_index_required".into(),
                            "register_provenance_subset".into(),
                        ],
                    ));
                }
                registers.clear();
                byte_sources.clear();
                hl_address = None;
                if let (Some(fact), Some(declaration)) = (
                    facts.last_mut().filter(|f| {
                        f.kind == "invoke"
                            && f.location.path == source.path
                            && f.location.line == number + 1
                    }),
                    declaration.as_ref(),
                ) {
                    fact.via.push(declaration.clone());
                }
            } else if operation == "add"
                && destination == "hl"
                && operands.len() == 2
                && matches!(operands[1].to_ascii_lowercase().as_str(), "bc" | "de")
            {
            } else if operation.starts_with("ret") || operation.starts_with('j') {
                registers.clear();
                byte_sources.clear();
                hl_address = None;
            } else if !matches!(
                operation.as_str(),
                "cmp" | "test" | "push" | "nop" | "bits" | "section"
            ) {
                if !matches!(
                    operation.as_str(),
                    "xor"
                        | "sub"
                        | "add"
                        | "and"
                        | "or"
                        | "pop"
                        | "inc"
                        | "dec"
                        | "shl"
                        | "shr"
                        | "sal"
                        | "sar"
                        | "not"
                        | "neg"
                ) {
                    registers.clear();
                } else if let Some((name, _)) = register(&destination) {
                    registers.remove(&name);
                }
                if matches!(destination.as_str(), "a" | "h" | "l") {
                    byte_sources.remove(&destination);
                }
                if matches!(destination.as_str(), "h" | "l" | "hl") {
                    hl_address = None;
                }
            }
        }
    }
    (facts, omitted)
}
