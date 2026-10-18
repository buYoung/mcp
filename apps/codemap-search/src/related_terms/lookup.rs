use super::normalize::{
    canonical_surface, inflection_bases, input_parts, spelling_alternative, word_key,
};
use super::prepare::PreparedDataset;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedInput {
    pub span: String,
    pub terms: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Resolution {
    pub mode: String,
    pub seeds: Vec<ResolvedInput>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Topic {
    pub topic: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Concept {
    pub concept: i64,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WordGroups {
    pub direct: Vec<String>,
    pub near: Vec<String>,
    pub same_topic: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SenseResult {
    pub span: String,
    pub field: String,
    pub field_label: String,
    pub topics: Vec<Topic>,
    pub concepts: Vec<Concept>,
    pub direct: Vec<String>,
    pub near: Vec<String>,
    pub same_topic: Vec<String>,
    /// Canonical words before cross-sense display de-duplication, excluding interpreted inputs.
    pub full_word_keys: Vec<String>,
    /// Preserve membership so overlay ordering can precede display de-duplication.
    #[serde(skip_serializing)]
    pub(crate) full_groups: WordGroups,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnsortedDirect {
    pub span: String,
    pub direct: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LookupResult {
    pub input: String,
    pub status: String,
    pub resolution: Resolution,
    pub senses: Vec<SenseResult>,
    pub unsorted_direct: Vec<UnsortedDirect>,
    #[serde(skip_serializing)]
    pub(crate) full_unsorted_direct: Vec<UnsortedDirect>,
    pub candidate_count: usize,
    /// Whole expression and component expressions attempted, including unmatched expressions.
    pub looked_up_inputs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LookupOptions {
    /// False matches the reference CLI default; only fallback parts are affected.
    pub skip_syntax_parts: bool,
}

/// Construction performs no I/O. One instance owns its lazy read-only SQLite connection
/// and bounded FIFO answer cache; callers should keep it on their lookup worker.
pub struct RelatedTermsLookup {
    prepared: PreparedDataset,
    options: LookupOptions,
    engine: Option<Engine>,
    answers: HashMap<String, LookupResult>,
    answer_order: VecDeque<String>,
}

impl RelatedTermsLookup {
    pub fn new(prepared: PreparedDataset) -> Self {
        Self::with_options(prepared, LookupOptions::default())
    }

    pub fn with_options(prepared: PreparedDataset, options: LookupOptions) -> Self {
        Self {
            prepared,
            options,
            engine: None,
            answers: HashMap::new(),
            answer_order: VecDeque::new(),
        }
    }

    fn engine(&mut self) -> Result<&mut Engine, String> {
        if self.engine.is_none() {
            self.engine = Some(Engine::new(self.prepared.open()?, self.options)?);
        }
        self.engine
            .as_mut()
            .ok_or_else(|| "lookup connection unavailable".into())
    }

    /// Evidence must read the lookup's open file, not a possibly replaced cache pathname.
    pub(super) fn connection(&mut self) -> Result<&Connection, String> {
        Ok(&self.engine()?.connection)
    }

    pub fn lookup(&mut self, input: &str) -> Result<LookupResult, String> {
        if let Some(answer) = self.answers.get(input) {
            return Ok(answer.clone());
        }
        let answer = self.engine()?.lookup(input)?;
        if self.answers.len() == 128 {
            if let Some(old) = self.answer_order.pop_front() {
                self.answers.remove(&old);
            }
        }
        self.answer_order.push_back(input.into());
        self.answers.insert(input.into(), answer.clone());
        Ok(answer)
    }

    pub fn lookup_batch(&mut self, terms: &[String]) -> Result<Vec<LookupResult>, String> {
        terms.iter().map(|term| self.lookup(term)).collect()
    }
}

#[derive(Clone)]
struct WordInfo {
    classified: bool,
    generated: bool,
    surface: String,
}
#[derive(Clone)]
struct TopicInfo {
    label: String,
    field_label: String,
    words: Vec<String>,
}
#[derive(Default)]
struct SenseWork {
    field: String,
    topic: String,
    concepts: BTreeSet<i64>,
    direct: Vec<String>,
    count: i64,
    own: bool,
    topic_size: usize,
}
struct Seed {
    span: String,
    keys: Vec<String>,
}
struct Engine {
    connection: Connection,
    words: HashMap<String, WordInfo>,
    topics: HashMap<String, TopicInfo>,
    siblings: bool,
    options: LookupOptions,
}

type PersonaRow = (i64, String, String, Option<i64>, i64, String);

impl Engine {
    fn new(connection: Connection, options: LookupOptions) -> Result<Self, String> {
        let rule: Option<String> = connection
            .query_row(
                "SELECT value FROM sense_meta WHERE key='input_sense_rule'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let direct = match rule {
            Some(rule) => serde_json::from_str::<serde_json::Value>(&rule)
                .map_err(|e| e.to_string())?["direct"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            None => "head_and_siblings".into(),
        };
        if !matches!(direct.as_str(), "head" | "head_and_siblings") {
            return Err(format!("unsupported input sense rule: {direct}"));
        }
        Ok(Self {
            connection,
            words: HashMap::new(),
            topics: HashMap::new(),
            siblings: direct == "head_and_siblings",
            options,
        })
    }

    fn strings(&self, sql: &str, key: &str) -> Result<Vec<String>, String> {
        let mut statement = self
            .connection
            .prepare_cached(sql)
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([key], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    fn word(&mut self, key: &str) -> Result<WordInfo, String> {
        if let Some(word) = self.words.get(key) {
            return Ok(word.clone());
        }
        let mut statement = self.connection.prepare_cached("SELECT
            EXISTS(SELECT 1 FROM personas WHERE term_id=(SELECT term_id FROM terms WHERE word_key=?1))
              OR EXISTS(SELECT 1 FROM input_senses WHERE word_key=?1),
            EXISTS(SELECT 1 FROM personas WHERE term_id=(SELECT term_id FROM terms WHERE word_key=?1) AND source_persona_count>0),
            coalesce((SELECT surface FROM terms WHERE word_key=?1),
              (SELECT coalesce(surface_override,word_key) FROM relations WHERE word_key=?1 ORDER BY term_id,rank LIMIT 1),
              (SELECT surface FROM input_relations WHERE related_word_key=?1 ORDER BY input_word_key,source_persona,rank LIMIT 1),?1)")
            .map_err(|e| e.to_string())?;
        let word = statement
            .query_row([key], |r| {
                Ok(WordInfo {
                    classified: r.get(0)?,
                    generated: r.get(1)?,
                    surface: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        if self.words.len() >= 16384 {
            self.words.clear();
        }
        self.words.insert(key.into(), word.clone());
        Ok(word)
    }

    fn surfaces(&mut self, keys: &[String]) -> Result<Vec<String>, String> {
        keys.iter()
            .map(|key| self.word(key).map(|w| w.surface))
            .collect()
    }

    fn topic(&mut self, topic: &str) -> Result<TopicInfo, String> {
        if let Some(info) = self.topics.get(topic) {
            return Ok(info.clone());
        }
        let (label, field_label) = self.connection.prepare_cached("SELECT t.label,f.label FROM topics t JOIN topic_fields tf USING(topic) JOIN fields f USING(field) WHERE t.topic=?1 AND tf.rank=0")
            .map_err(|e| e.to_string())?.query_row([topic], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
        let words = self.strings("SELECT DISTINCT t.word_key FROM personas p JOIN terms t USING(term_id) WHERE p.topic=?1", topic)?;
        let words = self.sort_words(words)?;
        let info = TopicInfo {
            label,
            field_label,
            words,
        };
        if self.topics.len() >= 256 {
            self.topics.clear();
        }
        self.topics.insert(topic.into(), info.clone());
        Ok(info)
    }

    fn sort_words(&mut self, mut words: Vec<String>) -> Result<Vec<String>, String> {
        words.sort();
        words.dedup();
        let mut labelled = words
            .into_iter()
            .map(|key| self.word(&key).map(|w| (w.surface.to_lowercase(), key)))
            .collect::<Result<Vec<_>, _>>()?;
        labelled.sort();
        Ok(labelled.into_iter().map(|(_, key)| key).collect())
    }

    fn evidence_rank(&self, span: &str, base: &str) -> Result<i64, String> {
        if !span.bytes().all(|c| c.is_ascii_alphabetic())
            || !["ing", "ed", "s"].iter().any(|s| span.ends_with(s))
        {
            return Ok(0);
        }
        self.connection.prepare_cached("SELECT coalesce(max(weight),0) FROM (
            SELECT CASE WHEN k.kind='form' THEN 2 ELSE 1 END AS weight FROM relations r
              JOIN terms t USING(term_id) JOIN relation_kinds k USING(kind_id) WHERE r.word_key=?1 AND t.word_key=?2
            UNION ALL SELECT 2 FROM relations r JOIN terms t USING(term_id) JOIN relation_kinds k USING(kind_id)
              WHERE t.word_key=?1 AND r.word_key=?2 AND k.kind='form')")
            .map_err(|e| e.to_string())?.query_row([span, base], |r| r.get(0)).map_err(|e| e.to_string())
    }

    fn resolve_span(&mut self, span: &str) -> Result<Vec<String>, String> {
        let key = word_key(span);
        let exact = self.word(&key)?;
        if exact.generated {
            return Ok(vec![key]);
        }
        let mut keys = BTreeSet::new();
        if exact.classified {
            keys.insert(key.clone());
        }
        let parts = input_parts(span);
        if let Some((last, head)) = parts.split_last() {
            let mut bases = Vec::new();
            for base in inflection_bases(last) {
                let base = format!("{}{base}", head.concat());
                let info = self.word(&base)?;
                let surface = canonical_surface(&info.surface);
                if info.classified
                    && surface.starts_with(|c: char| c.is_ascii_alphabetic())
                    && surface
                        .bytes()
                        .all(|b| b.is_ascii_alphabetic() || b == b' ')
                {
                    bases.push((self.evidence_rank(&parts.concat(), &base)?, base));
                }
            }
            let highest = bases.iter().map(|(rank, _)| *rank).max().unwrap_or(0);
            keys.extend(
                bases
                    .into_iter()
                    .filter(|(rank, _)| *rank == highest)
                    .map(|(_, base)| base),
            );
        }
        Ok(keys.into_iter().collect())
    }

    fn resolve(&mut self, query: &str, attempted: &mut Vec<String>) -> Result<Vec<Seed>, String> {
        attempted.push(query.into());
        let whole = self.resolve_span(query)?;
        let parts = input_parts(query);
        if !whole.is_empty() {
            // Expose identifier decomposition without widening a classified whole input.
            if parts.len() > 1 {
                for part in &parts {
                    if !attempted.contains(part) {
                        self.resolve_span(part)?;
                        attempted.push(part.clone());
                    }
                }
            }
            return Ok(vec![Seed {
                span: query.into(),
                keys: whole,
            }]);
        }
        let mut spans = BTreeSet::new();
        for size in 1..=parts.len().min(6) {
            for window in parts.windows(size) {
                if size == parts.len() {
                    continue;
                }
                if size == 1
                    && (window[0].len() == 1 || window[0].bytes().all(|b| b.is_ascii_digit()))
                {
                    continue;
                }
                spans.insert((std::cmp::Reverse(size), window.join(" ")));
            }
        }
        let skipped = if self.options.skip_syntax_parts {
            super::syntax::skipped_parts(query)
        } else {
            HashSet::new()
        };
        let mut used = HashSet::new();
        let mut seeds = Vec::new();
        for (_, span) in spans {
            if skipped.contains(&span) {
                continue;
            }
            attempted.push(span.clone());
            let keys = self.resolve_span(&span)?;
            if keys.is_empty() || span.split_whitespace().any(|token| used.contains(token)) {
                continue;
            }
            used.extend(span.split_whitespace().map(str::to_string));
            seeds.push(Seed { span, keys });
        }
        Ok(seeds)
    }

    fn personas(&self, key: &str, own: bool) -> Result<Vec<PersonaRow>, String> {
        let sql = if own {
            "SELECT p.persona,p.topic,tf.field,p.concept,p.source_persona_count,t.word_key FROM personas p
             JOIN terms t USING(term_id) JOIN topic_fields tf ON tf.topic=p.topic AND tf.rank=0 WHERE t.word_key=?1 ORDER BY p.persona"
        } else {
            "SELECT p.persona,p.topic,tf.field,p.concept,1,t.word_key FROM input_senses i JOIN personas p USING(persona)
             JOIN terms t USING(term_id) JOIN topic_fields tf ON tf.topic=p.topic AND tf.rank=0 WHERE i.word_key=?1 ORDER BY p.persona"
        };
        let mut statement = self
            .connection
            .prepare_cached(sql)
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([key], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    fn senses(&mut self, seed: &Seed) -> Result<Vec<SenseWork>, String> {
        let mut senses: BTreeMap<(String, String), SenseWork> = BTreeMap::new();
        for key in &seed.keys {
            for own in [true, false] {
                for (persona, topic, field, concept, count, head) in self.personas(key, own)? {
                    let sense = senses
                        .entry((field.clone(), topic.clone()))
                        .or_insert_with(|| SenseWork {
                            field,
                            topic,
                            ..Default::default()
                        });
                    sense.count += count;
                    sense.own |= own;
                    sense.concepts.extend(concept);
                    if !own {
                        sense.direct.push(head);
                        if self.siblings {
                            sense.direct.extend(self.strings("SELECT word_key FROM relations WHERE persona=?1 ORDER BY term_id,rank", &persona.to_string())?.into_iter().filter(|word| word != key));
                        }
                        let mut statement = self.connection.prepare_cached("SELECT related_word_key FROM input_relations WHERE input_word_key=?1 AND source_persona=?2 ORDER BY rank,related_word_key").map_err(|e| e.to_string())?;
                        let words = statement
                            .query_map(rusqlite::params![key, persona], |r| r.get::<_, String>(0))
                            .map_err(|e| e.to_string())?;
                        sense.direct.extend(
                            words
                                .collect::<Result<Vec<_>, _>>()
                                .map_err(|e| e.to_string())?,
                        );
                    }
                }
                if own {
                    let mut statement = self.connection.prepare_cached("SELECT tf.field,p.topic,r.word_key FROM relations r JOIN personas p USING(persona)
                        JOIN topic_fields tf ON tf.topic=p.topic AND tf.rank=0 WHERE r.term_id=(SELECT term_id FROM terms WHERE word_key=?1) ORDER BY r.rank").map_err(|e| e.to_string())?;
                    let rows = statement
                        .query_map([key], |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, String>(2)?,
                            ))
                        })
                        .map_err(|e| e.to_string())?;
                    for row in rows {
                        let (field, topic, word) = row.map_err(|e| e.to_string())?;
                        if let Some(sense) = senses.get_mut(&(field, topic)) {
                            sense.direct.push(word);
                        }
                    }
                }
            }
        }
        let mut senses: Vec<_> = senses.into_values().collect();
        for sense in &mut senses {
            sense.topic_size = self.topic(&sense.topic)?.words.len();
        }
        senses.sort_by(|a, b| {
            (
                a.count == 0,
                !a.own,
                -a.count,
                std::cmp::Reverse(a.topic_size),
                &a.field,
                &a.topic,
            )
                .cmp(&(
                    b.count == 0,
                    !b.own,
                    -b.count,
                    std::cmp::Reverse(b.topic_size),
                    &b.field,
                    &b.topic,
                ))
        });
        Ok(senses)
    }

    fn lookup(&mut self, input: &str) -> Result<LookupResult, String> {
        let canonical = canonical_surface(input);
        let mut looked_up_inputs = Vec::new();
        let mut seeds = self.resolve(&canonical, &mut looked_up_inputs)?;
        let mut excluded = HashSet::from([word_key(&canonical)]);
        if let Some(alternative) = spelling_alternative(&word_key(&canonical)) {
            let extra = self.resolve(alternative, &mut looked_up_inputs)?;
            if !extra.is_empty() {
                excluded.insert(word_key(alternative));
                seeds.extend(extra);
            }
        }
        for seed in &seeds {
            excluded.insert(word_key(&seed.span));
            excluded.extend(seed.keys.iter().cloned());
        }
        let mut result = LookupResult {
            input: input.into(),
            status: "no_match".into(),
            resolution: Resolution {
                mode: if seeds.is_empty() {
                    "none"
                } else if seeds.iter().any(|s| {
                    s.span == canonical
                        || spelling_alternative(&word_key(&canonical)) == Some(s.span.as_str())
                }) {
                    "whole"
                } else {
                    "parts"
                }
                .into(),
                seeds: Vec::new(),
            },
            senses: Vec::new(),
            unsorted_direct: Vec::new(),
            full_unsorted_direct: Vec::new(),
            candidate_count: 0,
            looked_up_inputs,
        };
        let mut groups: Vec<(Vec<String>, Vec<String>, Vec<String>)> = Vec::new();
        let mut unsorted = Vec::new();
        for seed in &seeds {
            result.resolution.seeds.push(ResolvedInput {
                span: seed.span.clone(),
                terms: self.surfaces(&seed.keys)?,
            });
            for sense in self.senses(seed)? {
                let topic = self.topic(&sense.topic)?;
                let mut near = Vec::new();
                let mut concepts = Vec::new();
                for concept in &sense.concepts {
                    near.extend(self.strings("SELECT DISTINCT t.word_key FROM personas p JOIN terms t USING(term_id) WHERE p.concept=?1", &concept.to_string())?);
                    let name = self
                        .connection
                        .prepare_cached("SELECT name FROM concepts WHERE concept=?1")
                        .map_err(|e| e.to_string())?
                        .query_row([concept], |r| r.get(0))
                        .map_err(|e| e.to_string())?;
                    concepts.push(Concept {
                        concept: *concept,
                        name,
                    });
                }
                let near = self.sort_words(near)?;
                let full: BTreeSet<String> = sense
                    .direct
                    .iter()
                    .chain(&near)
                    .chain(&topic.words)
                    .filter(|key| !excluded.contains(*key))
                    .cloned()
                    .collect();
                result.senses.push(SenseResult {
                    span: seed.span.clone(),
                    field: sense.field,
                    field_label: topic.field_label,
                    topics: vec![Topic {
                        topic: sense.topic,
                        label: topic.label,
                    }],
                    concepts,
                    direct: Vec::new(),
                    near: Vec::new(),
                    same_topic: Vec::new(),
                    full_word_keys: full.into_iter().collect(),
                    full_groups: WordGroups {
                        direct: self.place(&sense.direct, &mut excluded.clone())?,
                        near: self.place(&near, &mut excluded.clone())?,
                        same_topic: self.place(&topic.words, &mut excluded.clone())?,
                    },
                });
                groups.push((sense.direct, near, topic.words));
            }
            let mut direct = Vec::new();
            for key in &seed.keys {
                direct.extend(self.strings("SELECT word_key FROM relations WHERE term_id=(SELECT term_id FROM terms WHERE word_key=?1) AND persona IS NULL ORDER BY rank", key)?);
            }
            unsorted.push(direct);
            result.unsorted_direct.push(UnsortedDirect {
                span: seed.span.clone(),
                direct: Vec::new(),
            });
        }
        let mut placed = excluded.clone();
        for (sense, (direct, _, _)) in result.senses.iter_mut().zip(&groups) {
            sense.direct = self.place(direct, &mut placed)?;
        }
        for (group, words) in result.unsorted_direct.iter_mut().zip(&unsorted) {
            result.full_unsorted_direct.push(UnsortedDirect {
                span: group.span.clone(),
                direct: self.place(words, &mut excluded.clone())?,
            });
            group.direct = self.place(words, &mut placed)?;
        }
        for (sense, (_, near, _)) in result.senses.iter_mut().zip(&groups) {
            sense.near = self.place(near, &mut placed)?;
        }
        for (sense, (_, _, topic)) in result.senses.iter_mut().zip(&groups) {
            sense.same_topic = self.place(topic, &mut placed)?;
        }
        result.candidate_count = placed.len() - excluded.len();
        result.status = if result.candidate_count > 0 {
            "ok"
        } else if !seeds.is_empty() {
            "no_sense"
        } else {
            "no_match"
        }
        .into();
        Ok(result)
    }

    fn place(
        &mut self,
        words: &[String],
        placed: &mut HashSet<String>,
    ) -> Result<Vec<String>, String> {
        let fresh: Vec<_> = words
            .iter()
            .filter(|key| placed.insert((*key).clone()))
            .cloned()
            .collect();
        self.surfaces(&fresh)
    }
}
