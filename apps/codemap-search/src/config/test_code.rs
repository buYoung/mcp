//! Per-language test rules. Each configured list replaces its inherited list.

use std::collections::BTreeMap;
use std::path::Path;

pub type LanguageTestPatterns = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TestCodeRules {
    pub file_patterns: Vec<String>,
    pub attributes: LanguageTestPatterns,
    pub decorators: LanguageTestPatterns,
    pub calls: LanguageTestPatterns,
}

fn language_patterns(entries: &[(&str, &[&str])]) -> LanguageTestPatterns {
    entries
        .iter()
        .map(|(language, patterns)| {
            (
                (*language).to_string(),
                patterns
                    .iter()
                    .map(|pattern| (*pattern).to_string())
                    .collect(),
            )
        })
        .collect()
}

impl Default for TestCodeRules {
    fn default() -> Self {
        Self {
            file_patterns: [
                "**/tests/**",
                "**/test/**",
                "**/__tests__/**",
                "test_*.py",
                "*_test.*",
                "*.test.*",
                "*_spec.*",
                "*.spec.*",
                "*Test.java",
                "*Tests.java",
                "*IT.java",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            attributes: language_patterns(&[
                (
                    "rust",
                    &[
                        "test",
                        "tokio::test",
                        "async_std::test",
                        "rstest",
                        "rstest::rstest",
                        "cfg(test)",
                    ],
                ),
                (
                    "java",
                    &[
                        "Test",
                        "ParameterizedTest",
                        "RepeatedTest",
                        "TestFactory",
                        "TestTemplate",
                        "Nested",
                        "BeforeEach",
                        "AfterEach",
                        "BeforeAll",
                        "AfterAll",
                    ],
                ),
                (
                    "kotlin",
                    &[
                        "Test",
                        "ParameterizedTest",
                        "RepeatedTest",
                        "BeforeTest",
                        "AfterTest",
                        "BeforeEach",
                        "AfterEach",
                    ],
                ),
                (
                    "csharp",
                    &[
                        "Fact",
                        "Theory",
                        "Test",
                        "TestCase",
                        "TestCaseSource",
                        "TestFixture",
                        "SetUp",
                        "TearDown",
                        "OneTimeSetUp",
                        "OneTimeTearDown",
                    ],
                ),
                ("swift", &["Test", "Suite"]),
                ("php", &["Test"]),
            ]),
            decorators: language_patterns(&[(
                "python",
                &[
                    "pytest.fixture",
                    "pytest.mark.*",
                    "unittest.skip",
                    "unittest.skipIf",
                    "unittest.skipUnless",
                    "unittest.expectedFailure",
                ],
            )]),
            calls: language_patterns(&[
                (
                    "javascript",
                    &[
                        "describe",
                        "describe.*",
                        "it",
                        "it.*",
                        "test",
                        "test.*",
                        "suite",
                        "suite.*",
                    ],
                ),
                (
                    "typescript",
                    &[
                        "describe",
                        "describe.*",
                        "it",
                        "it.*",
                        "test",
                        "test.*",
                        "suite",
                        "suite.*",
                    ],
                ),
                ("dart", &["test", "group", "testWidgets"]),
                ("ruby", &["describe", "context", "it", "specify"]),
                ("powershell", &["Describe", "Context", "It"]),
            ]),
        }
    }
}

#[derive(Default)]
pub(super) struct TestCodeLayer {
    pub file_patterns: Option<Vec<String>>,
    pub attributes: LanguageTestPatterns,
    pub decorators: LanguageTestPatterns,
    pub calls: LanguageTestPatterns,
}

pub(super) fn merge_test_code_rules(repo: TestCodeLayer, global: TestCodeLayer) -> TestCodeRules {
    let mut rules = TestCodeRules::default();
    rules.file_patterns = repo
        .file_patterns
        .or(global.file_patterns)
        .unwrap_or(rules.file_patterns);
    rules.attributes.extend(global.attributes);
    rules.attributes.extend(repo.attributes);
    rules.decorators.extend(global.decorators);
    rules.decorators.extend(repo.decorators);
    rules.calls.extend(global.calls);
    rules.calls.extend(repo.calls);
    rules
}

pub(super) fn parse_patterns(
    value: &toml::Value,
    key: &str,
    path: &Path,
    is_file_pattern: bool,
) -> Option<Vec<String>> {
    let mut patterns = super::as_string_array(value, key, path)?;
    if is_file_pattern {
        for pattern in &mut patterns {
            *pattern = pattern.replace('\\', "/");
        }
    }
    for pattern in &patterns {
        let is_invalid_path = is_file_pattern
            && (Path::new(pattern).is_absolute()
                || (pattern.as_bytes().get(1) == Some(&b':')
                    && pattern.as_bytes()[0].is_ascii_alphabetic())
                || pattern
                    .replace('\\', "/")
                    .split('/')
                    .any(|part| part == ".."));
        if pattern.trim().is_empty()
            || pattern.starts_with('!')
            || is_invalid_path
            || globset::GlobBuilder::new(pattern)
                .literal_separator(is_file_pattern)
                .backslash_escape(false)
                .build()
                .is_err()
        {
            super::warn(&format!("config '{key}' contains an invalid pattern '{pattern}': {} — using inherited rules", path.display()));
            return None;
        }
    }
    Some(patterns)
}

pub(super) fn parse_languages(value: &toml::Value, key: &str, path: &Path) -> LanguageTestPatterns {
    let mut rules = LanguageTestPatterns::new();
    let Some(table) = value.as_table() else {
        super::warn(&format!(
            "config '{key}' must be a language table: {} — using inherited rules",
            path.display()
        ));
        return rules;
    };
    for (language, value) in table {
        if !crate::lang::is_known_language(language) {
            super::warn(&format!(
                "unknown test-rule language '{language}' in '{key}': {} — ignored",
                path.display()
            ));
            continue;
        }
        if let Some(patterns) = parse_patterns(value, &format!("{key}.{language}"), path, false) {
            rules.insert(language.clone(), patterns);
        }
    }
    rules
}
