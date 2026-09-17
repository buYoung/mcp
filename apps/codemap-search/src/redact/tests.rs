use super::*;

#[test]
fn test_credentials_are_hidden_without_changing_safe_code_or_line_breaks() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let _request = begin_request();
    let cases = [
        (
            "{\"password\":\n  \"next-line-secret\"}",
            "next-line-secret",
        ),
        (
            "password = r\"\"\"\nraw-multiline-secret\n\"\"\"",
            "raw-multiline-secret",
        ),
        ("password = @\"first\"\"last-secret\";", "last-secret"),
        (
            "API_KEY=plain-secret-value\nSAFE=value\n",
            "plain-secret-value",
        ),
        (
            "const PASSWORD: &str = \"한글-비밀번호\";\r\n",
            "한글-비밀번호",
        ),
        (
            "{\"access_token\":\"arbitrary-credential\",\"port\":8080}",
            "arbitrary-credential",
        ),
        (
            "Authorization: Bearer bearer-value-12345",
            "bearer-value-12345",
        ),
        (
            "postgres://reader:database-password@localhost/app",
            "database-password",
        ),
        (
            "value = 'ghp_abcdefghijklmnopqrstuvwxyz0123456789'",
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
        ),
        (
            "password = \"\"\"\nmultiline-secret-value\n\"\"\"\npublic = 'visible'\n",
            "multiline-secret-value",
        ),
        (
            "private_key: |\n  yaml-secret-value\npublic: visible\n",
            "yaml-secret-value",
        ),
        (
            "-----BEGIN PRIVATE KEY-----\npem-secret-value\n-----END PRIVATE KEY-----\n",
            "pem-secret-value",
        ),
        (
            "const PASSWORD: &str = r#\"raw-secret-value\"#;",
            "raw-secret-value",
        ),
        ("password='x'", "'x'"),
    ];
    for (input, secret) in cases {
        let masked = source(input);
        assert!(!masked.contains(secret), "credential remained visible");
        assert!(masked.len() <= input.len());
        assert_eq!(masked.matches('\n').count(), input.matches('\n').count());
        assert_eq!(masked.matches('\r').count(), input.matches('\r').count());
    }
    let safe = "const token = process.env.TOKEN;\nlet password = load_password();\nconst token_count = 12;\nconst message = \"hello\";\n// Basic conditional paths remain visible.\n";
    assert_eq!(source(safe), safe);
}

#[test]
fn test_response_keeps_source_locations_and_protocol_structure() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let _request = begin_request();
    let original = "password = \"\"\"\ninterior-secret\n\"\"\"\npublic = 42\n";
    let rendered = source(original)
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:>6}→{line}\n", i + 1))
        .collect::<String>();
    let mut value = serde_json::json!({
        "content": [{"type":"text", "text":rendered}],
        "other": {"code": -32602, "message": "API_KEY=error-secret-value"}
    });
    response(&mut value);
    let text = value["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("2→[REDACTED]") && text.contains("4→public = 42"),
        "{text}"
    );
    assert_eq!(value["content"][0]["type"], "text");
    assert_eq!(value["other"]["code"], -32602);
    assert!(!value.to_string().contains("interior-secret"));
    assert!(!value.to_string().contains("error-secret-value"));
    let first = value.clone();
    response(&mut value);
    assert_eq!(value, first);
}

#[test]
fn test_configuration_layers_and_cli_do_not_silently_enable_or_disable_redaction() {
    let repo = tempfile::tempdir().unwrap();
    let global = tempfile::tempdir().unwrap();
    std::fs::create_dir(repo.path().join(".codemap")).unwrap();
    std::fs::write(
        global.path().join("config.toml"),
        "[tool_output]\nis_redact_enabled=false\n",
    )
    .unwrap();
    assert!(!crate::config::load(repo.path(), global.path()).is_redact_enabled);
    let repo_config = repo.path().join(".codemap/config.toml");
    std::fs::write(&repo_config, "[tool_output]\nis_redact_enabled=true\n").unwrap();
    assert!(crate::config::load(repo.path(), global.path()).is_redact_enabled);
    std::fs::write(&repo_config, "[tool_output]\nis_redact_enabled='invalid'\n").unwrap();
    assert!(!crate::config::load(repo.path(), global.path()).is_redact_enabled);

    let input = "password='unmasked-outside-mcp'";
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    assert_eq!(source(input), input);
    {
        let _request = begin_request();
        assert_ne!(source(input), input);
        let disabled = crate::config::ResolvedConfig {
            is_redact_enabled: false,
            ..Default::default()
        };
        let _disabled = crate::config::pin_test_config(disabled);
        assert_eq!(source(input), input);
    }
    assert_eq!(source(input), input);
}
#[test]
fn test_syntax_masks_values_and_preserves_references_and_types() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let _request = begin_request();
    let cases = [
        ("config.ts", "type Options = { password: string };\nconst password: string = externalValue;\nconst config = { apiKey: 'sensitive-literal', visible: 'public-literal' };", "sensitive-literal", "password: string = externalValue"),
        ("config.py", "password: str = external_value\ndef connect(api_key: str = 'sensitive-literal'): pass\n", "sensitive-literal", "password: str = external_value"),
        ("config.rs", "const PASSWORD: &str = ENV_VALUE;\nconst API_KEY: &str = r#\"sensitive-literal\"#;", "sensitive-literal", "PASSWORD: &str = ENV_VALUE"),
        ("config.go", "package demo\nvar password = externalValue\nvar apiKey = `sensitive-literal`\n", "sensitive-literal", "password = externalValue"),
        ("config.json", "{\"api-key\": \"sensitive-literal\", \"public\": \"public-literal\"}", "sensitive-literal", "public-literal"),
        ("config.yaml", "api_key: sensitive-literal\npublic: public-literal\n", "sensitive-literal", "public-literal"),
        ("config.toml", "api_key = 'sensitive-literal'\npublic = 'public-literal'\n", "sensitive-literal", "public-literal"),
        (".env", "PASSWORD=first; sensitive-literal # comment\nPUBLIC=public-literal\n", "sensitive-literal", "PUBLIC=public-literal"),
        ("config.ini", "password=first; sensitive-literal\npublic=public-literal\n", "sensitive-literal", "public=public-literal"),
        ("broken.ts", "const password = 'sensitive-literal'\nconst = ???;\nconst publicValue = 'public-literal';", "sensitive-literal", "public-literal"),
    ];
    for (path, input, secret, safe) in cases {
        let masked = in_file(Path::new(path), input);
        assert!(!masked.contains(secret), "{path}: {masked}");
        assert!(masked.contains(safe), "{path}: {masked}");
        let mut response_value = serde_json::json!({"content":[{"type":"text", "text":masked}]});
        response(&mut response_value);
        assert_eq!(
            response_value["content"][0]["text"],
            masked.as_ref(),
            "{path}"
        );
    }
}

#[test]
fn test_original_spans_preserve_adjacent_literals_and_unicode() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let _request = begin_request();
    let input = "const 설명 = {password: '비밀-문자열', message: 'public-literal'};\r\n";
    let scan = SourceScan::new(Path::new("config.ts"), input);
    let secret_start = input.find("비밀-문자열").unwrap();
    assert!(scan
        .detections
        .iter()
        .any(|detection| detection.rule_id == "field.sensitive"
            && detection.kind == DetectionKind::SensitiveField
            && detection.range == (secret_start..secret_start + "비밀-문자열".len())));
    for (value, is_secret) in [("비밀-문자열", true), ("public-literal", false)] {
        let literal = crate::parser::ExtractedLiteral {
            text: value.into(),
            line: 1,
        };
        let rendered = scan.literal(input, &literal);
        assert_eq!(rendered == value, !is_secret);
    }
    assert!(!scan
        .literal(
            input,
            &crate::parser::ExtractedLiteral {
                text: "stale-secret".into(),
                line: 9
            }
        )
        .contains("stale-secret"));
    let masked = scan.render(input);
    assert!(masked.ends_with("\r\n"));
    assert!(masked.len() <= input.len());
}

#[test]
fn test_vendor_catalog_and_overlap_cover_the_whole_original_value() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let _request = begin_request();
    for value in [
        "sk_live_abcdefghijklmnop0123456789",
        "glpat-abcdefghijklmnop0123456789",
        "npm_abcdefghijklmnop0123456789",
        "xoxp-abcdefghijklmnop0123456789",
        "SG.abcdefghijklmnop012345.abcdefghijklmnop0123456789",
    ] {
        assert!(!source(value).contains(value));
    }
    let input = format!("password = 'sk-proj-{}-tail-of-secret'", "a".repeat(240));
    assert!(!source(&input).contains("tail-of-secret"));
    assert_eq!(mask_ranges("abcdefghijklmnop", vec![0..8, 4..16]), MARKER);
}

#[test]
fn test_custom_rules_exact_exceptions_and_per_key_fallback() {
    let repo = tempfile::tempdir().unwrap();
    let global = tempfile::tempdir().unwrap();
    std::fs::create_dir(repo.path().join(".codemap")).unwrap();
    std::fs::write(
        global.path().join("config.toml"),
        r#"
[redact]
sensitive_fields = ['internalCredential']
rules = [{ id = 'custom.acme', pattern = 'ACME_[A-Z0-9]+' }]
exceptions = [{ rule_id = 'custom.acme', value = 'ACME_EXAMPLE' }]
"#,
    )
    .unwrap();
    let path = repo.path().join(".codemap/config.toml");
    std::fs::write(
        &path,
        "[redact]\nrules = [{ id = 'custom.invalid', pattern = '[' }]\n",
    )
    .unwrap();
    let config = crate::config::load(repo.path(), global.path());
    assert_eq!(config.redact.rules[0].id, "custom.acme");
    let _config = crate::config::pin_test_config(config);
    let _request = begin_request();
    let input = "const config = { internal_credential: 'private-value', safe: 'ACME_EXAMPLE', other: 'ACME_SECRET' };";
    let masked = in_file(Path::new("config.ts"), input);
    assert!(masked.contains("ACME_EXAMPLE"));
    assert!(!masked.contains("ACME_SECRET") && !masked.contains("private-value"));
    // An exception never suppresses another matching rule or a longer value.
    assert!(!source("ACME_EXAMPLEPLUS").contains("ACME_EXAMPLE"));
    assert!(!source("password='ACME_EXAMPLE'").contains("ACME_EXAMPLE"));
    std::fs::write(&path, "[redact]\nrules=[]\nexceptions=[]\n").unwrap();
    let config = crate::config::load(repo.path(), global.path());
    assert!(config.redact.rules.is_empty() && config.redact.exceptions.is_empty());
    assert_eq!(config.redact.sensitive_fields, ["internalcredential"]);
}

#[test]
fn test_interpolated_concatenated_and_unrecognized_values_retain_coverage() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let _request = begin_request();
    for (path, source, secrets, safe) in [
        ("value.ts", "const apiKey = `sensitive-prefix-${reference}-sensitive-suffix`;", vec!["sensitive-prefix", "sensitive-suffix"], "${reference}"),
        ("value.py", "password = 'sensitive-prefix-' + reference + '-sensitive-suffix'\n", vec!["sensitive-prefix", "sensitive-suffix"], "reference"),
        ("value.cs", "class Demo { const string password = @\"sensitive-prefix-secret\"; }", vec!["sensitive-prefix-secret"], "class Demo"),
        ("value.java", "class Demo { String password = \"sensitive-prefix-secret\"; String token = reference; }", vec!["sensitive-prefix-secret"], "token = reference"),
        ("value.json", r#"{"api\u005fkey":"sensitive-prefix-secret", "safe":"public-value"}"#, vec!["sensitive-prefix-secret"], "public-value"),
        ("value.yaml", "password: | # details\n  sensitive-prefix-secret\n  sensitive-suffix-secret\nsafe: public-value\n", vec!["sensitive-prefix-secret", "sensitive-suffix-secret"], "safe: public-value"),
    ] {
        let output = in_file(Path::new(path), source);
        for secret in secrets { assert!(!output.contains(secret), "{path}: {output}"); }
        assert!(output.contains(safe), "{path}: {output}");
    }
}

#[test]
fn test_stale_partial_and_ambiguous_literals_are_conservative() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let _request = begin_request();
    let input = "const password = 'same-value'; const safe = 'same-value'; const publicValue = 'longer-value';";
    let scan = SourceScan::new(Path::new("config.ts"), input);
    for value in ["same-value", "longer", "obsolete-value"] {
        assert_ne!(
            scan.literal(
                input,
                &crate::parser::ExtractedLiteral {
                    text: value.into(),
                    line: 1
                }
            ),
            value
        );
    }
    assert_eq!(
        scan.literal(
            input,
            &crate::parser::ExtractedLiteral {
                text: "longer-value".into(),
                line: 1
            }
        ),
        "longer-value"
    );
}

#[test]
fn test_custom_capture_ranges_and_member_names() {
    let mut config = crate::config::ResolvedConfig::default();
    config
        .redact
        .sensitive_fields
        .push("internalcredential".into());
    config.redact.rules.push(crate::config::redact::RedactRule {
        id: "custom.capture".into(),
        pattern: regex::Regex::new(r"LABEL=(?P<secret>VALUE_[A-Z]+)").unwrap(),
    });
    let _config = crate::config::pin_test_config(config);
    let _request = begin_request();
    let source = "config.internalCredential = 'private-value'; const note = 'LABEL=VALUE_SECRET';";
    let scan = SourceScan::new(Path::new("config.ts"), source);
    let output = scan.render(source);
    assert!(!output.contains("private-value") && !output.contains("VALUE_SECRET"));
    assert!(output.contains("LABEL="));
    assert!(scan
        .detections
        .iter()
        .any(|d| d.rule_id == "custom.capture" && d.kind == DetectionKind::Custom));
}
