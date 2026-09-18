use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    entity: String,
    text: String,
    expected_len: usize,
    ranges: Option<Vec<[usize; 2]>>,
    source: String,
}

fn cases() -> Vec<Case> {
    include_str!("cases.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn test_large_normal_source_has_no_pii_candidates() {
    let input: String = (1..=10_000)
        .map(|line| format!("  total += {line} % 7; // public-row-{line:05}\n"))
        .collect();
    let mut total = 0;
    for entry in catalog() {
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        let started = std::time::Instant::now();
        let found = rule.detect(&input, true);
        let elapsed = started.elapsed().as_millis();
        if elapsed >= 20 {
            eprintln!(
                "PII scan {}: {elapsed}ms, {} candidates",
                entry.definition.entity,
                found.len()
            );
        }
        total += found.len();
    }
    assert_eq!(total, 0);
}

#[test]
fn test_large_mixed_source_scanning_preserves_utf8_ranges() {
    let examples = cases();
    let mut lines: Vec<_> = (1..=10_000)
        .map(|line| format!("  total += {line} % 7; // public-row-{line:05}"))
        .collect();
    for (index, entry) in catalog().iter().enumerate() {
        let case = examples
            .iter()
            .find(|case| case.entity == entry.definition.entity && case.expected_len == 1)
            .unwrap();
        for cycle in 0..10 {
            lines[6 + (cycle * 90 + index) * 11] = format!(
                "const row_{cycle}_{index} = {{ {}: '{}' }}; // 🌍 개인정보 datos بيانات",
                case.entity, case.text
            );
        }
    }
    let input = lines.join("\r\n");
    let mut count = 0;
    for entry in catalog() {
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        let started = std::time::Instant::now();
        let ranges = rule.detect(&input, true);
        let elapsed = started.elapsed().as_millis();
        if elapsed >= 20 {
            eprintln!(
                "Mixed PII scan {}: {elapsed}ms, {} candidates",
                entry.definition.entity,
                ranges.len()
            );
        }
        count += ranges.len();
        assert!(
            ranges
                .iter()
                .all(|range| input.get(range.clone()).is_some()),
            "{}",
            entry.definition.entity
        );
        let masked = crate::redact::transform::mask_ranges(&input, ranges);
        assert!(masked.len() <= input.len());
        assert_eq!(masked.lines().count(), input.lines().count());
    }
    assert!(count >= 900, "only {count} candidates");
}

#[test]
fn test_repeated_candidates_preserve_all_original_ranges() {
    let cases = cases();
    let mut failures = Vec::new();
    let mut checked = 0;
    for entry in catalog() {
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        for case in cases.iter().filter(|case| {
            case.entity == entry.definition.entity
                && case.expected_len == 1
                && case.ranges.as_deref() == Some(&[[0, case.text.len()]])
        }) {
            for separator in [",", ";", "|", "\n", "\r\n", "\t", " "] {
                let input = format!(
                    "{}{}{}{}{}",
                    case.text, separator, case.text, separator, case.text
                );
                let stride = case.text.len() + separator.len();
                let expected: Vec<_> = (0..3)
                    .map(|index| index * stride..index * stride + case.text.len())
                    .collect();
                let actual = rule.find(&input);
                let is_covered = expected.iter().all(|expected| {
                    actual
                        .iter()
                        .any(|actual| actual.start <= expected.start && actual.end >= expected.end)
                });
                // Some separators are legal address characters; VAT permits a
                // trailing separator. Those cases assert complete protection.
                let has_exact_boundaries = matches!(
                    case.entity.as_str(),
                    "URL" | "EMAIL_ADDRESS" | "IT_VAT_CODE"
                ) || actual == expected;
                if !is_covered || !has_exact_boundaries {
                    failures.push(format!(
                        "{} {} separator={separator:?}: expected {expected:?}, got {actual:?}",
                        case.entity, case.source
                    ));
                }
                checked += 1;
            }
        }
    }
    eprintln!("repeated PII boundary cases: {checked}");
    assert!(
        failures.is_empty(),
        "{} boundary failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn test_all_pii_rules_preserve_normal_code_identifiers() {
    let input = "export function digest(sha256: string, utf16: string) {\n  const timeoutMs = 10000;\n  const response = request.contentType;\n  return sha256 + utf16 + response;\n}\n";
    let mut matches = Vec::new();
    for entry in catalog() {
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        for range in rule.detect(input, true) {
            matches.push(format!("{}: {:?}", entry.definition.entity, &input[range]));
        }
    }
    assert!(
        matches.is_empty(),
        "normal code matched:\n{}",
        matches.join("\n")
    );
}

#[test]
fn test_context_is_limited_to_the_related_value() {
    use crate::redact::{begin_request, in_file, response};
    let mut config = crate::config::ResolvedConfig::default();
    config.redact.pii_entities = catalog()
        .iter()
        .map(|entry| entry.definition.entity.clone())
        .collect();
    let _config = crate::config::pin_test_config(config);
    let _request = begin_request();
    let source = "export const postalCode = '10000'; const timeoutMs = 20000;\nconst memberId = 'ABC123456789'; const digest = 'sha256';\nconst postcode =\n  '20000';\nconst memberIdReference = sha256;\nconst member = sha256;\nconst country = 'DE';\nconst timeout = 30000;\nconst website = 'example.org'; const response = request.contentType;\nconst url = 'https://example.org/path';\n";
    let expected = "export const postalCode = '*****'; const timeoutMs = 20000;\nconst memberId = '[REDACTED]'; const digest = 'sha256';\nconst postcode =\n  '*****';\nconst memberIdReference = sha256;\nconst member = sha256;\nconst country = 'DE';\nconst timeout = 30000;\nconst website = '[REDACTED]'; const response = request.contentType;\nconst url = '[REDACTED]';\n";
    let output = in_file(std::path::Path::new("safe.ts"), source);
    assert_eq!(output, expected);
    let mut value = serde_json::json!({"content":[{"type":"text","text":output}]});
    response(&mut value);
    assert_eq!(value["content"][0]["text"], expected);
}

#[test]
fn test_url_requires_complete_host_labels_and_preserves_source_quotes() {
    let entry = catalog()
        .iter()
        .find(|entry| entry.definition.entity == "URL")
        .unwrap();
    let rule = entry
        .compiled
        .get_or_init(|| CompiledRule::new(&entry.definition));
    assert!(rule
        .find("request.contentType; output.is_redact_enabled; template.companyName")
        .is_empty());
    for suffix in ["company", "community", "com", "co", "in", "international"] {
        let value = format!("https://example.{suffix}");
        let input = format!("const url = '{value}';");
        let ranges = rule.detect(&input, true);
        assert_eq!(ranges, vec![13..13 + value.len()], "{input}");
    }
}

#[test]
fn test_rejected_iban_prefix_does_not_swallow_the_next_value() {
    let entry = catalog()
        .iter()
        .find(|entry| entry.definition.entity == "IBAN_CODE")
        .unwrap();
    let rule = entry
        .compiled
        .get_or_init(|| CompiledRule::new(&entry.definition));
    let value = "BE68539007547034";
    let input = format!("X{value} {value}");
    assert_eq!(rule.find(&input), vec![value.len() + 2..input.len()]);
}

#[test]
fn test_candidate_localization_preserves_unicode_boundaries() {
    let entry = catalog()
        .iter()
        .find(|entry| entry.definition.entity == "DE_BSNR")
        .unwrap();
    let rule = entry
        .compiled
        .get_or_init(|| CompiledRule::new(&entry.definition));
    for (prefix, suffix, is_expected) in [
        ("🌍 ", " datos", true),
        ("한", "", false),
        ("", "한", false),
        ("é", "", false),
        ("", "\u{301}", false),
        ("\0", "\0", true),
    ] {
        let input = format!("{prefix}021234568{suffix}");
        let expected = if is_expected {
            std::iter::once(prefix.len()..prefix.len() + 9).collect::<Vec<_>>()
        } else {
            vec![]
        };
        assert_eq!(rule.find(&input), expected, "{input:?}");
    }
}

#[test]
fn test_context_labels_do_not_borrow_from_variable_references() {
    for (entity, label, value) in [
        ("DE_PLZ", "postalCode", "10115"),
        ("IT_PASSPORT", "passportNumber", "AA1234567"),
        ("KR_PASSPORT", "여권번호", "M123A4567"),
        ("IT_DRIVER_LICENSE", "driverLicense", "AA0123456B"),
        ("US_BANK_NUMBER", "bankAccount", "945456787654"),
        ("US_SSN", "socialSecurityNumber", "321-54-9876"),
    ] {
        let entry = catalog()
            .iter()
            .find(|entry| entry.definition.entity == entity)
            .unwrap();
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        let input = format!("const {label} = '{value}';");
        let start = input.find(value).unwrap();
        assert_eq!(
            rule.detect(&input, true),
            vec![start..start + value.len()],
            "{entity}"
        );
    }
    let entry = catalog()
        .iter()
        .find(|entry| entry.definition.entity == "DE_PLZ")
        .unwrap();
    let rule = entry
        .compiled
        .get_or_init(|| CompiledRule::new(&entry.definition));
    assert!(rule
        .detect("const timeoutMs = postalCode.length + 10000;", true)
        .is_empty());
}

#[test]
fn test_mutated_unicode_candidates_never_break_output_ranges() {
    let examples = cases();
    let mut checked = 0;
    for entry in catalog() {
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        let case = examples
            .iter()
            .find(|case| case.entity == entry.definition.entity && case.expected_len > 0)
            .unwrap();
        for offset in case
            .text
            .char_indices()
            .map(|(offset, _)| offset)
            .step_by(3)
        {
            for replacement in ["é", "한", "🙂", "١", "\0", "\r\n"] {
                let mut input = case.text.clone();
                input.insert_str(offset, replacement);
                let ranges = rule.find(&input);
                assert!(
                    ranges
                        .iter()
                        .all(|range| input.get(range.clone()).is_some()),
                    "{}",
                    case.entity
                );
                let masked = crate::redact::transform::mask_ranges(&input, ranges);
                assert!(masked.len() <= input.len());
                assert_eq!(masked.matches('\n').count(), input.matches('\n').count());
                assert_eq!(masked.matches('\r').count(), input.matches('\r').count());
                checked += 1;
            }
        }
    }
    eprintln!("Unicode mutation cases: {checked}");
}

#[test]
fn test_unicode_email_labels_preserve_combining_characters() {
    let entry = catalog()
        .iter()
        .find(|entry| entry.definition.entity == "EMAIL_ADDRESS")
        .unwrap();
    let rule = entry
        .compiled
        .get_or_init(|| CompiledRule::new(&entry.definition));
    let value = "u@cafe\u{301}.com";
    let text = format!("연락처: {value}");
    let found = rule.find(&text);
    assert_eq!(found.len(), 1);
    assert_eq!(&text[found[0].clone()], value);
}

#[test]
fn test_decimal_digit_validation_keeps_original_utf8_ranges() {
    let entry = catalog()
        .iter()
        .find(|entry| entry.definition.entity == "CREDIT_CARD")
        .unwrap();
    let rule = entry
        .compiled
        .get_or_init(|| CompiledRule::new(&entry.definition));
    let value = "4١١١١١١١١١١١١١١١";
    let text = format!("🌍 بيانات '{value}' public");
    let found = rule.find(&text);
    assert_eq!(found.len(), 1);
    assert_eq!(&text[found[0].clone()], value);
    let masked = crate::redact::transform::mask_ranges(&text, found);
    assert_eq!(masked, "🌍 بيانات '[REDACTED]' public");
}

#[test]
fn test_healthcare_field_adapters_require_their_own_label() {
    for (entity, label, value) in [
        (
            "US_PRIOR_AUTHORIZATION_NUMBER",
            "priorAuthorizationNumber",
            "987654321",
        ),
        ("US_CLAIM_NUMBER", "claimNumber", "1234567890123"),
        ("US_PRESCRIPTION_NUMBER", "prescriptionNumber", "1234567"),
        ("US_REFERRAL_NUMBER", "referralNumber", "2025001234"),
        ("US_PROVIDER_TAX_ID", "providerTaxId", "12-3456789"),
    ] {
        let entry = catalog()
            .iter()
            .find(|entry| entry.definition.entity == entity)
            .unwrap();
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        let input = format!("const {label} = '{value}';");
        let start = input.find(value).unwrap();
        assert_eq!(rule.detect(&input, true), vec![start..start + value.len()]);
        assert!(
            rule.detect(&input, false).is_empty(),
            "formatted output must not infer {entity} labels"
        );
        assert!(rule
            .detect(&format!("const itemCount = '{value}';"), true)
            .is_empty());
        assert!(rule
            .detect(&format!("const {label}Count = '{value}';"), true)
            .is_empty());
        assert!(rule
            .detect(
                &format!("const {label} = other; const itemCount = '{value}';"),
                true
            )
            .is_empty());
    }
}

#[test]
fn test_application_fixture_fields_match_their_selected_entity() {
    let fixtures = [
        ("merchant_service.ts", "merchant_service.pii.json"),
        ("merchant_service.go", "merchant_service.go.pii.json"),
        ("merchant_service.rs", "merchant_service.rs.pii.json"),
        ("MerchantService.java", "MerchantService.java.pii.json"),
        (
            "merchant_dashboard.html",
            "merchant_dashboard.html.pii.json",
        ),
    ];
    for (source_name, manifest_name) in fixtures {
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/redact_app");
        let source = std::fs::read_to_string(directory.join(source_name)).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(directory.join(manifest_name)).unwrap())
                .unwrap();
        let lines: Vec<_> = source.lines().collect();
        let mut failures = Vec::new();
        for span in manifest["spans"].as_array().unwrap() {
            let entity = span["entity"].as_str().unwrap();
            let entry = catalog()
                .iter()
                .find(|entry| entry.definition.entity == entity)
                .unwrap();
            let rule = entry
                .compiled
                .get_or_init(|| CompiledRule::new(&entry.definition));
            let line = span["line"].as_u64().unwrap() as usize;
            let start = span["column_bytes"].as_u64().unwrap() as usize;
            let value = span["value"].as_str().unwrap();
            let expected = start..start + value.len();
            assert_eq!(lines[line - 1].get(expected.clone()), Some(value));
            let actual = rule.detect(lines[line - 1], true);
            if !actual
                .iter()
                .any(|range| range.start <= expected.start && range.end >= expected.end)
            {
                failures.push(format!(
                    "{source_name}: {entity} field {} at line {line}: expected {expected:?}, got {actual:?}",
                    span["field"]
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} real-field failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn test_presidio_pattern_examples() {
    let cases = cases();
    let mut failures = Vec::new();
    for entry in catalog() {
        assert!(
            cases
                .iter()
                .any(|case| case.entity == entry.definition.entity),
            "no cases for {}",
            entry.definition.entity
        );
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        for case in cases
            .iter()
            .filter(|case| case.entity == entry.definition.entity)
        {
            let actual = rule.find(&case.text);
            let actual_ranges: Vec<_> = actual
                .iter()
                .map(|range| [range.start, range.end])
                .collect();
            if actual.len() != case.expected_len
                || case
                    .ranges
                    .as_ref()
                    .is_some_and(|expected| *expected != actual_ranges)
            {
                failures.push(format!(
                    "{} {}: expected {:?} ({}), got {:?}",
                    case.entity, case.source, case.ranges, case.expected_len, actual_ranges
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn test_every_entity_preserves_utf8_coordinates() {
    let cases = cases();
    let prefix = "🌍 개인정보 بيانات: ";
    for entry in catalog() {
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        let case = cases
            .iter()
            .find(|case| case.entity == entry.definition.entity && case.expected_len > 0)
            .unwrap_or_else(|| panic!("no positive case for {}", entry.definition.entity));
        let original = rule.find(&case.text);
        let shifted = rule.find(&format!("{prefix}{}", case.text));
        let expected: Vec<_> = original
            .iter()
            .map(|range| range.start + prefix.len()..range.end + prefix.len())
            .collect();
        assert_eq!(shifted, expected, "{}", case.entity);
    }
}

#[test]
fn test_selection_and_exceptions_reach_all_redaction_stages() {
    use crate::redact::{begin_request, in_file, response, source};
    let mut config = crate::config::ResolvedConfig::default();
    let input = "const card = '4111111111111111'; const contact = 'info@presidio.site'; const password = 'hidden-password';";
    let _request = begin_request();
    {
        let _config = crate::config::pin_test_config(config.clone());
        let output = in_file(std::path::Path::new("data.ts"), input);
        assert!(output.contains("4111111111111111") && output.contains("info@presidio.site"));
        assert!(!output.contains("hidden-password"));
    }
    config.redact.pii_entities = vec!["CREDIT_CARD".into(), "EMAIL_ADDRESS".into()];
    config
        .redact
        .exceptions
        .push(crate::config::redact::RedactException {
            rule_id: "pii.email-address".into(),
            value: "info@presidio.site".into(),
        });
    {
        let _config = crate::config::pin_test_config(config.clone());
        let output = in_file(std::path::Path::new("data.ts"), input);
        assert!(!output.contains("4111111111111111") && !output.contains("hidden-password"));
        assert!(output.contains("info@presidio.site"));
        let mut value = serde_json::json!({"content":[{"type":"text", "text":output}], "number":4111111111111111u64});
        response(&mut value);
        assert_eq!(value["number"], 4111111111111111u64);
        assert!(value["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("info@presidio.site"));
        assert!(!source("password='info@presidio.site'").contains("info@presidio.site"));
    }
    config.is_redact_enabled = false;
    let _config = crate::config::pin_test_config(config);
    assert_eq!(in_file(std::path::Path::new("data.ts"), input), input);
}

#[test]
fn test_pii_configuration_inheritance_and_per_key_fallback() {
    let repo = tempfile::tempdir().unwrap();
    let global = tempfile::tempdir().unwrap();
    std::fs::create_dir(repo.path().join(".codemap")).unwrap();
    std::fs::write(
        global.path().join("config.toml"),
        "[redact]\npii_entities=['CREDIT_CARD']\n",
    )
    .unwrap();
    let path = repo.path().join(".codemap/config.toml");
    std::fs::write(
        &path,
        "[redact]\npii_entities=['UNSUPPORTED']\nsensitive_fields=['account']\n",
    )
    .unwrap();
    let config = crate::config::load(repo.path(), global.path());
    assert_eq!(config.redact.pii_entities, ["CREDIT_CARD"]);
    assert_eq!(config.redact.sensitive_fields, ["account"]);
    std::fs::write(&path, "[redact]\npii_entities=[]\n").unwrap();
    assert!(crate::config::load(repo.path(), global.path())
        .redact
        .pii_entities
        .is_empty());
}

#[test]
fn test_pattern_boundaries_preserve_adjacent_values_and_labels() {
    let rule = |entity: &str| {
        let entry = catalog()
            .iter()
            .find(|entry| entry.definition.entity == entity)
            .unwrap();
        entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition))
    };
    let text = "name='900101-1234567', other='900101-3234567'";
    let values: Vec<_> = rule("KR_RRN")
        .find(text)
        .into_iter()
        .map(|range| &text[range])
        .collect();
    assert_eq!(values, ["900101-1234567", "900101-3234567"]);
    let text = "Rx #1234567; Rx #7654321";
    let values: Vec<_> = rule("US_PRESCRIPTION_NUMBER")
        .find(text)
        .into_iter()
        .map(|range| &text[range])
        .collect();
    assert_eq!(values, ["1234567", "7654321"]);
    assert!(rule("CA_SIN").find("046-454 286").is_empty());
    assert!(rule("MAC_ADDRESS").find("00:1A-2B:3C:4D:5E").is_empty());
}
