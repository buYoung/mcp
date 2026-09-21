use grep_matcher::{LineTerminator, Match, Matcher, NoCaptures, NoError};
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};
use std::{ffi::{c_char, c_void, CStr}, ptr::NonNull};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Span { start: usize, end: usize }

extern "C" {
    fn poc_re2_new(pattern: *const c_char, length: usize, error: *mut c_char, capacity: usize) -> *mut c_void;
    fn poc_re2_free(regex: *mut c_void);
    fn poc_re2_capture_count(regex: *const c_void) -> usize;
    fn poc_re2_match(regex: *const c_void, data: *const c_char, length: usize,
                     start: usize, spans: *mut Span, count: usize) -> bool;
}

// Owns the native object; no Send/Sync implementation: the bridge reuses capture
// scratch space and this benchmark intentionally runs on one thread.
pub struct Re2 { pointer: NonNull<c_void>, captures: usize, is_line: bool }

impl Re2 {
    fn new(pattern: &str, is_line: bool) -> Result<Self, String> {
        let mut error = [0i8; 2048];
        // SAFETY: inputs and writable error buffer live for the call. The native
        // constructor copies the pattern and returns an owned object or null.
        let pointer = unsafe { poc_re2_new(pattern.as_ptr().cast(), pattern.len(), error.as_mut_ptr(), error.len()) };
        let pointer = NonNull::new(pointer).ok_or_else(|| unsafe { CStr::from_ptr(error.as_ptr()).to_string_lossy().into_owned() })?;
        let captures = unsafe { poc_re2_capture_count(pointer.as_ptr()) };
        Ok(Self { pointer, captures, is_line })
    }

    fn matches(&self, text: &[u8], at: usize, spans: &mut [Span]) -> bool {
        // SAFETY: the object remains owned by self; native code only borrows text
        // and writes at most spans.len() entries during this synchronous call.
        unsafe { poc_re2_match(self.pointer.as_ptr(), text.as_ptr().cast(), text.len(), at, spans.as_mut_ptr(), spans.len()) }
    }

    fn visit(&self, text: &str, captures: bool, mut emit: impl FnMut(usize, usize, usize)) {
        let mut whole_span = [Span::default()];
        let mut capture_spans = if captures { vec![Span::default(); self.captures] } else { Vec::new() };
        let spans = if captures { capture_spans.as_mut_slice() } else { &mut whole_span };
        let mut at = 0;
        let mut last_end = None;
        while at <= text.len() && self.matches(text.as_bytes(), at, spans) {
            let whole = spans[0];
            if whole.start == whole.end {
                // Match Rust's UTF-8 iterator: advance one code point and skip
                // empty matches immediately adjacent to the preceding match.
                at = whole.end + text[whole.end..].chars().next().map_or(1, char::len_utf8);
                if last_end == Some(whole.end) { continue; }
            } else {
                at = whole.end;
            }
            last_end = Some(whole.end);
            for (group, span) in spans.iter().enumerate() {
                emit(group, span.start, span.end);
            }
        }
    }
}

impl Drop for Re2 {
    fn drop(&mut self) { unsafe { poc_re2_free(self.pointer.as_ptr()) } }
}

impl Matcher for Re2 {
    type Captures = NoCaptures;
    type Error = NoError;
    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<Match>, NoError> {
        let mut span = [Span::default()];
        Ok(self.matches(haystack, at, &mut span).then(|| Match::new(span[0].start, span[0].end)))
    }
    fn new_captures(&self) -> Result<NoCaptures, NoError> { Ok(NoCaptures::new()) }
    fn line_terminator(&self) -> Option<LineTerminator> {
        // Only the closed set of benchmark grep patterns is passed in this
        // mode. None can consume LF; this is not a general production adapter.
        self.is_line.then(|| LineTerminator::byte(b'\n'))
    }
}

pub enum Engine {
    Regex(regex::Regex),
    Fancy(fancy_regex::Regex),
    Re2(Re2),
    Grep(grep_regex::RegexMatcher),
}

pub fn compile(name: &str, pattern: &str, is_line: bool) -> Result<Engine, String> {
    match name {
        "regex" => regex::Regex::new(pattern).map(Engine::Regex).map_err(|e| e.to_string()),
        "fancy_regex" => fancy_regex::Regex::new(pattern).map(Engine::Fancy).map_err(|e| e.to_string()),
        "re2" => Re2::new(pattern, is_line).map(Engine::Re2),
        "grep_regex" => grep_regex::RegexMatcherBuilder::new()
            .case_insensitive(false).multi_line(false).dot_matches_new_line(false)
            .line_terminator(Some(b'\n')).build(pattern)
            .map(Engine::Grep).map_err(|e| e.to_string()),
        _ => Err(format!("unknown engine: {name}")),
    }
}

#[derive(serde::Deserialize)]
pub struct Document { pub name: String, pub text: String }

struct FancyMatcher<'a>(&'a fancy_regex::Regex);
impl Matcher for FancyMatcher<'_> {
    type Captures = NoCaptures;
    type Error = String;
    fn find_at(&self, haystack: &[u8], at: usize) -> Result<Option<Match>, String> {
        // Version 0.19.2 accepts byte slices; preserve the original buffer and
        // offset just as RE2::Match and grep-regex do.
        self.0.find_from_pos(haystack, at).map(|value| value.map(|m| Match::new(m.start(), m.end())))
            .map_err(|e| e.to_string())
    }
    fn new_captures(&self) -> Result<NoCaptures, String> { Ok(NoCaptures::new()) }
    fn line_terminator(&self) -> Option<LineTerminator> { Some(LineTerminator::byte(b'\n')) }
}

struct MatchSink<'a, F> { document: usize, emit: &'a mut F }
impl<F: FnMut([usize; 4])> Sink for MatchSink<'_, F> {
    type Error = std::io::Error;
    fn matched(&mut self, _: &Searcher, matched: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        let start = matched.absolute_byte_offset() as usize;
        (self.emit)([self.document, matched.line_number().unwrap() as usize, start, start + matched.bytes().len()]);
        Ok(true)
    }
}

impl Engine {
    pub fn visit(&self, mode: &str, documents: &[Document], mut emit: impl FnMut([usize; 4])) -> Result<(), String> {
        if mode == "grep" {
            // Same searcher settings as tools/grep.rs with default zero context.
            // File buffers are preloaded; output assembly/AST/redaction is excluded.
            let mut searcher = SearcherBuilder::new().line_number(true)
                .binary_detection(BinaryDetection::quit(0)).build();
            for (document, source) in documents.iter().enumerate() {
                let sink = MatchSink { document, emit: &mut emit };
                match self {
                    Self::Re2(re) => searcher.search_slice(re, source.text.as_bytes(), sink),
                    Self::Grep(re) => searcher.search_slice(re, source.text.as_bytes(), sink),
                    Self::Fancy(re) => searcher.search_slice(FancyMatcher(re), source.text.as_bytes(), sink),
                    _ => unreachable!("grep uses its actual RegexMatcher baseline"),
                }.map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        for (document, source) in documents.iter().enumerate() {
            let text = source.text.as_str();
            if mode == "is_match" {
                let found = match self {
                    Self::Regex(re) => re.is_match(text),
                    Self::Fancy(re) => re.is_match(text).map_err(|e| e.to_string())?,
                    Self::Re2(re) => re.matches(text.as_bytes(), 0, &mut []),
                    Self::Grep(re) => re.is_match(text.as_bytes()).unwrap(),
                };
                if found { emit([document, 0, 0, 0]); }
            } else {
                match self {
                    Self::Regex(re) if mode == "captures" => {
                        for captures in re.captures_iter(text) {
                            for (group, value) in captures.iter().enumerate() {
                                let (start, end) = value.map_or((usize::MAX, usize::MAX), |m| (m.start(), m.end()));
                                emit([document, group, start, end]);
                            }
                        }
                    }
                    Self::Regex(re) => {
                        for found in re.find_iter(text) { emit([document, 0, found.start(), found.end()]); }
                    }
                    Self::Fancy(re) if mode == "captures" => {
                        for captures in re.captures_iter(text) {
                            let captures = captures.map_err(|e| e.to_string())?;
                            for group in 0..captures.len() {
                                let (start, end) = captures.get(group).map_or((usize::MAX, usize::MAX), |m| (m.start(), m.end()));
                                emit([document, group, start, end]);
                            }
                        }
                    }
                    Self::Fancy(re) => {
                        for found in re.find_iter(text) {
                            let found = found.map_err(|e| e.to_string())?;
                            emit([document, 0, found.start(), found.end()]);
                        }
                    }
                    Self::Re2(re) => re.visit(text, mode == "captures", |group, start, end| emit([document, group, start, end])),
                    _ => unreachable!(),
                }
            }
        }
        Ok(())
    }
}
