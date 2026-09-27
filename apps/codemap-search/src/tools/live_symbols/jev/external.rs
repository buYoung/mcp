//! Bounded cross-file support over source revalidated before inference. An indexed
//! spelling match is a candidate, never proof of dispatch or of an absent caller.
use super::{context, CapturedFile};
use crate::parser::{CallSite, ExtractedFile};
use crate::tools::search::jev::{
    simple_name, FilterEntity, FilterInput, FilterPolicy, MAX_COMPLETE_BODY_BYTES,
    MAX_LINKS_PER_QUESTION,
};
use std::collections::{HashMap, HashSet};

fn has_stopped(policy: &FilterPolicy) -> bool {
    policy
        .cancel
        .as_ref()
        .is_some_and(|cancel| cancel.is_cancelled())
        || policy
            .deadline_at
            .is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
}

fn note_gap(entity: &mut FilterEntity, reason: &str) {
    entity.has_missing_context = true;
    let note = format!("\nCross-file evidence gap: {reason}\n");
    if !entity.supporting_context.contains(&note) {
        if entity.supporting_context.len().saturating_add(note.len()) <= MAX_COMPLETE_BODY_BYTES {
            entity.supporting_context.push_str(&note);
        } else {
            entity.is_context_clipped = true;
        }
    }
}

fn may_reference(owner: Option<&str>, file: &ExtractedFile, call: &CallSite) -> bool {
    let Some(navigation) = &file.navigation else {
        return false;
    };
    if let Some(owner) = owner {
        call.receiver
            .as_deref()
            .is_some_and(|receiver| simple_name(receiver) == simple_name(owner))
            || navigation.imports.iter().any(|import| {
                import
                    .imported_name
                    .as_deref()
                    .unwrap_or(&import.local_name)
                    == owner
            })
    } else {
        call.receiver.is_none()
            || navigation
                .imports
                .iter()
                .any(|import| call.receiver.as_deref() == Some(import.local_name.as_str()))
    }
}

fn append_support(entity: &mut FilterEntity, excerpt: context::SupportingContext, role: &str) {
    let text = format!(
        "\n{role} (target identity unverified; check source bindings):{}",
        excerpt.text
    );
    if entity.supporting_context.len().saturating_add(text.len()) > MAX_COMPLETE_BODY_BYTES {
        entity.is_context_clipped = true;
        note_gap(
            entity,
            "A supporting excerpt exceeds the evidence byte budget.",
        );
        return;
    }
    entity.supporting_context.push_str(&text);
    entity.has_missing_context |= excerpt.has_missing;
    entity.is_context_clipped |= excerpt.is_clipped;
}

struct EvidenceFiles<'a> {
    indexed: &'a [ExtractedFile],
    captured: &'a [CapturedFile],
}

fn add_dependencies(
    input: &mut FilterInput,
    indices: &[usize],
    sources: EvidenceFiles<'_>,
    loaded: &mut HashMap<String, Option<CapturedFile>>,
    exclusions: &crate::callers::test_code::TestCodeFilter,
    policy: &FilterPolicy,
    remaining: &mut usize,
) {
    let EvidenceFiles {
        indexed: files,
        captured: captured_files,
    } = sources;
    // Collect only explicit call/import spellings from the candidate source, then
    // scan the index once. Import/type evidence can expose a constructor's role.
    let mut wanted: HashMap<String, Vec<(usize, Option<&CallSite>)>> = HashMap::new();
    for &index in indices {
        let entity = &input.entities[index];
        let Some(file) = captured_files.get(entity.file_index) else {
            continue;
        };
        let Some(navigation) = &file.file.navigation else {
            continue;
        };
        let contains = |line| entity.symbol.start_line <= line && line <= entity.symbol.end_line;
        for call in navigation
            .calls
            .iter()
            .filter(|call| contains(call.range.start_line))
        {
            let name = navigation
                .imports
                .iter()
                .find(|import| call.receiver.is_none() && import.local_name == call.name)
                .and_then(|import| import.imported_name.as_deref())
                .unwrap_or(&call.name);
            wanted
                .entry(simple_name(name).into())
                .or_default()
                .push((index, Some(call)));
        }
        for import in navigation.imports.iter().filter(|import| {
            navigation.references.iter().any(|reference| {
                contains(reference.range.start_line) && reference.name == import.local_name
            })
        }) {
            wanted
                .entry(
                    import
                        .imported_name
                        .as_deref()
                        .unwrap_or(&import.local_name)
                        .into(),
                )
                .or_default()
                .push((index, None));
        }
    }
    let mut counts = vec![0usize; input.entities.len()];
    let mut shown = HashSet::new();
    for file in files
        .iter()
        .filter(|file| !exclusions.is_file_excluded(&file.file_path))
    {
        for symbol in &file.symbols {
            if has_stopped(policy) {
                for &index in indices {
                    note_gap(
                        &mut input.entities[index],
                        "Dependency scan stopped before covering the index.",
                    );
                }
                return;
            }
            let Some(candidates) = wanted.get(simple_name(&symbol.name)) else {
                continue;
            };
            if exclusions.is_excluded(&file.file_path, &symbol.range) {
                continue;
            }
            for &(index, call) in candidates {
                let entity = &mut input.entities[index];
                if entity.path == file.file_path {
                    continue;
                }
                let current = &captured_files[entity.file_index];
                let is_candidate = match call {
                    Some(call) => {
                        crate::tools::search::jev::is_callable_kind(&symbol.kind)
                            && may_reference(symbol.owner.as_deref(), &current.file, call)
                    }
                    None => symbol.owner.is_none(),
                };
                if !is_candidate
                    || !shown.insert((
                        index,
                        &file.file_path,
                        symbol.range.start_line,
                        &symbol.name,
                    ))
                {
                    continue;
                }
                if counts[index] == MAX_LINKS_PER_QUESTION {
                    note_gap(entity, "Additional dependencies exceed the link budget.");
                    entity.is_context_clipped = true;
                    continue;
                }
                if *remaining == 0 {
                    note_gap(
                        entity,
                        "Dependency candidates exceed the shared analysis budget.",
                    );
                    entity.is_context_clipped = true;
                    continue;
                }
                *remaining -= 1;
                counts[index] += 1;
                let captured = captured_files
                    .iter()
                    .find(|source| source.path == file.file_path);
                if captured.is_none() && !loaded.contains_key(&file.file_path) {
                    if loaded.len() == MAX_LINKS_PER_QUESTION {
                        note_gap(
                            entity,
                            "Additional sources exceed the per-response file budget.",
                        );
                        entity.is_context_clipped = true;
                        continue;
                    }
                    loaded.insert(file.file_path.clone(), load(file, exclusions));
                }
                let Some(source) =
                    captured.or_else(|| loaded.get(&file.file_path).and_then(Option::as_ref))
                else {
                    note_gap(entity, "A dependency source is unavailable within read permissions or parsing limits.");
                    continue;
                };
                let Some(definition) = source.file.symbols.iter().position(|current| {
                    current.name == symbol.name
                        && current.owner == symbol.owner
                        && current.kind == symbol.kind
                        && current.range == symbol.range
                }) else {
                    note_gap(
                        entity,
                        "An indexed dependency could not be revalidated in current source.",
                    );
                    continue;
                };
                append_support(
                    entity,
                    context::supporting_declaration(source, definition),
                    "Possible cross-file dependency",
                );
            }
        }
    }
}

fn load(
    file: &ExtractedFile,
    exclusions: &crate::callers::test_code::TestCodeFilter,
) -> Option<CapturedFile> {
    let path = crate::workspace::resolve_for_filesystem_tool(
        &file.file_path,
        crate::workspace::FilesystemTool::Read,
    )
    .ok()?;
    let config = crate::config::get();
    let max_bytes = config
        .max_file_size
        .min(super::super::callable::input_byte_cap() as u64);
    if std::fs::metadata(&path).ok()?.len() > max_bytes {
        return None;
    }
    let mut source = crate::workspace::read_source_for_parse(&path)?.into_bytes();
    exclusions.mask_source(&file.file_path, &mut source);
    CapturedFile::new(&file.file_path, &String::from_utf8(source).ok()?)
}

fn excerpt(file: &CapturedFile, call: &CallSite) -> Option<(usize, context::SupportingContext)> {
    let navigation = file.file.navigation.as_ref()?;
    // The index only selects work. Validate the actual call and its enclosing body
    // in the newly captured source rather than trusting stale indexed coordinates.
    if !navigation.calls.iter().any(|current| current == call) {
        return None;
    }
    let bounds = file
        .bounds
        .iter()
        .filter(|bounds| {
            bounds.start <= call.range.start_line && call.range.end_line_inclusive() <= bounds.end
        })
        .min_by_key(|bounds| bounds.end - bounds.start)?;
    Some((
        bounds.start,
        context::supporting_declaration(file, bounds.symbol_index),
    ))
}

pub(super) fn augment(
    input: &mut FilterInput,
    engine: &crate::index::EngineSupervisor,
    captured_files: &[CapturedFile],
    policy: &FilterPolicy,
) {
    let indices = input.judgeable();
    if indices.is_empty() {
        return;
    }
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for &index in &indices {
        by_name
            .entry(simple_name(&input.entities[index].symbol.name).into())
            .or_default()
            .push(index);
    }
    let snapshot = engine.published_snapshot();
    let files = snapshot.codemap();
    let is_unavailable = engine.is_warming() || engine.is_dead() || engine.last_error().is_some();
    if is_unavailable {
        for index in indices {
            note_gap(
                &mut input.entities[index],
                "The external index is unavailable for evidence capture.",
            );
        }
        return;
    }
    let root = std::env::current_dir().unwrap_or_default();
    let exclusions = crate::callers::test_code::TestCodeFilter::from_config(&root);
    let config = crate::config::get();
    let mut remaining = config.navigation_callsite_budget;
    let mut counts = vec![0usize; input.entities.len()];
    let mut pending = Vec::new();
    'files: for file in files
        .iter()
        .filter(|file| !exclusions.is_file_excluded(&file.file_path))
    {
        let Some(navigation) = &file.navigation else {
            continue;
        };
        for call in &navigation.calls {
            if has_stopped(policy) {
                for &index in &indices {
                    note_gap(
                        &mut input.entities[index],
                        "Caller scan stopped before covering the index.",
                    );
                    input.entities[index].is_context_clipped = true;
                }
                break 'files;
            }
            let name = navigation
                .imports
                .iter()
                .find(|import| call.receiver.is_none() && import.local_name == call.name)
                .and_then(|import| import.imported_name.as_deref())
                .unwrap_or(&call.name);
            let Some(candidates) = by_name.get(simple_name(name)) else {
                continue;
            };
            if exclusions.is_excluded(&file.file_path, &call.range) {
                continue;
            }
            for &index in candidates {
                let entity = &mut input.entities[index];
                if entity.path == file.file_path
                    || !may_reference(entity.symbol.owner.as_deref(), file, call)
                {
                    continue;
                }
                if counts[index] >= MAX_LINKS_PER_QUESTION {
                    note_gap(entity, "Additional callers exceed the link budget.");
                    entity.is_context_clipped = true;
                    continue;
                }
                // The configured budget bounds candidate analysis, not cheap name
                // lookup through unrelated index entries. Keep the caller's deadline
                // check above even when no names match.
                if remaining == 0 {
                    note_gap(
                        entity,
                        "Caller candidates exceed the shared analysis budget.",
                    );
                    entity.is_context_clipped = true;
                    continue;
                }
                remaining -= 1;
                counts[index] += 1;
                pending.push((index, file, call));
            }
        }
    }
    // Capture before classification. Finding a possible relationship must not remove
    // its candidate from evaluation or make this evidence path unreachable.
    let mut loaded: HashMap<String, Option<CapturedFile>> = HashMap::new();
    let mut shown = HashSet::new();
    for (index, file, call) in pending {
        let entity = &mut input.entities[index];
        if has_stopped(policy) {
            note_gap(
                entity,
                "The caller's deadline or cancellation stopped evidence capture.",
            );
            continue;
        }
        let captured = captured_files
            .iter()
            .find(|source| source.path == file.file_path);
        if captured.is_none() && !loaded.contains_key(&file.file_path) {
            if loaded.len() >= MAX_LINKS_PER_QUESTION {
                entity.is_context_clipped = true;
                note_gap(
                    entity,
                    "Additional sources exceed the per-response file budget.",
                );
                continue;
            }
            loaded.insert(file.file_path.clone(), load(file, &exclusions));
        }
        let Some(source) =
            captured.or_else(|| loaded.get(&file.file_path).and_then(Option::as_ref))
        else {
            note_gap(
                entity,
                "A supporting source is unavailable within read permissions or parsing limits.",
            );
            continue;
        };
        let Some((start, excerpt)) = excerpt(source, call) else {
            note_gap(
                entity,
                "An indexed call could not be revalidated in current source.",
            );
            continue;
        };
        if !shown.insert((index, file.file_path.as_str(), start)) {
            continue;
        }
        append_support(entity, excerpt, "Possible cross-file caller");
    }
    add_dependencies(
        input,
        &indices,
        EvidenceFiles {
            indexed: &files,
            captured: captured_files,
        },
        &mut loaded,
        &exclusions,
        policy,
        &mut remaining,
    );
}
