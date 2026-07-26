use crate::e2e::helpers::{create_mock_repo, run_cli, McpClient};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const OPTIONAL_TOKEN: &str = "group_boundary_token";
const JUNK_TOKEN: &str = "optional_group_junk_token";
const SEARCH_GROUPS: &[(&str, &str)] = &[
    ("shell_group_boundary_token", "shell/"),
    ("infrastructure_group_boundary_token", "infrastructure/"),
    ("interface_group_boundary_token", "interface/"),
    ("build_group_boundary_token", "build_defs/"),
];

const OPTIONAL_FILES: &[(&str, &str)] = &[
    (
        "shell/script.sh",
        "shell_group_boundary_token() { :; }\n",
    ),
    (
        "shell/script.bash",
        "shell_group_boundary_token() { :; }\n",
    ),
    (
        "shell/script.zsh",
        "shell_group_boundary_token() { :; }\n",
    ),
    (
        "infrastructure/main.hcl",
        "variable \"infrastructure_group_boundary_token\" {}\n",
    ),
    (
        "infrastructure/main.tf",
        "variable \"infrastructure_group_boundary_token\" {}\n",
    ),
    (
        "infrastructure/values.tfvars",
        "infrastructure_group_boundary_token = true\n",
    ),
    (
        "infrastructure/Dockerfile",
        "FROM scratch\nLABEL infrastructure_group_boundary_token=true\n",
    ),
    (
        "infrastructure/default.nix",
        "{ infrastructure_group_boundary_token = true; }\n",
    ),
    (
        "interface/service.proto",
        "syntax = \"proto3\";\nmessage InterfaceGroupBoundaryToken {}\n// interface_group_boundary_token\n",
    ),
    (
        "interface/schema.graphql",
        "type InterfaceGroupBoundaryToken { value: String }\n# interface_group_boundary_token\n",
    ),
    (
        "interface/query.gql",
        "query InterfaceGroupBoundaryToken { interface_group_boundary_token }\n",
    ),
    (
        "build_defs/Makefile",
        "build_group_boundary_token:\n\t@true\n",
    ),
    (
        "build_defs/rules.mk",
        "build_group_boundary_token:\n\t@true\n",
    ),
    (
        "build_defs/CMakeLists.txt",
        "function(build_group_boundary_token)\nendfunction()\n",
    ),
    (
        "build_defs/module.cmake",
        "function(build_group_boundary_token)\nendfunction()\n",
    ),
    (
        "build_defs/BUILD",
        "build_group_boundary_token = True\n",
    ),
    (
        "build_defs/BUILD.bazel",
        "build_group_boundary_token = True\n",
    ),
    (
        "build_defs/defs.bzl",
        "def build_group_boundary_token():\n    pass\n",
    ),
];

const JUNK_FILES: &[(&str, &str)] = &[
    ("ignored/notes.txt", JUNK_TOKEN),
    ("ignored/Cargo.lock", JUNK_TOKEN),
    ("ignored/app.js.map", JUNK_TOKEN),
    ("ignored/app.min.js", JUNK_TOKEN),
    ("ignored/app.bundle.js", JUNK_TOKEN),
];

fn config(is_enabled: bool) -> String {
    format!(
        "# codemap-config-version: 5\n\
         [refresh]\n\
         index_staleness_ms = 3600000\n\
         watch_debounce_ms = 100\n\
         [search]\n\
         result_threshold = 50\n\
         search_overview_file_limit = 50\n\
         [language_support]\n\
         is_shell_support_enabled = {is_enabled}\n\
         is_infrastructure_support_enabled = {is_enabled}\n\
         is_interface_support_enabled = {is_enabled}\n\
         is_build_support_enabled = {is_enabled}\n"
    )
}

fn fixture(is_enabled: bool, include_junk: bool) -> tempfile::TempDir {
    let config = config(is_enabled);
    let mut files = vec![(".codemap/config.toml", config.as_str())];
    files.extend_from_slice(OPTIONAL_FILES);
    if include_junk {
        files.extend_from_slice(JUNK_FILES);
    }
    create_mock_repo(&files).unwrap()
}

fn response_text(response: &Value) -> &str {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
}

fn tool_call(name: &str, arguments: Value) -> Value {
    json!({ "name": name, "arguments": arguments })
}

fn cli_stdout(args: &[&str], cwd: &Path) -> String {
    let assert = run_cli(args, cwd).success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn assert_contains_all_optional_files(text: &str) {
    for (path, _) in OPTIONAL_FILES {
        assert!(text.contains(path), "missing {path}: {text}");
    }
}

fn assert_contains_no_optional_files(text: &str) {
    for (path, _) in OPTIONAL_FILES {
        assert!(!text.contains(path), "unexpected {path}: {text}");
    }
}

fn assert_contains_group_files(text: &str, path_prefix: &str) {
    for (path, _) in OPTIONAL_FILES
        .iter()
        .filter(|(path, _)| path.starts_with(path_prefix))
    {
        assert!(text.contains(path), "missing {path}: {text}");
    }
}

#[tokio::test]
async fn test_optional_language_groups_toggle_index_discovery_but_keep_direct_access() {
    let temp = fixture(false, false);
    run_cli(&["index"], temp.path()).success();

    for (query, _) in SEARCH_GROUPS {
        let disabled_cli_search = cli_stdout(&["search", query], temp.path());
        assert_contains_no_optional_files(&disabled_cli_search);
    }
    for (path, _) in OPTIONAL_FILES {
        run_cli(&["codemap", "--path", path], temp.path()).failure();
        run_cli(&["parse", path], temp.path()).success();
    }

    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    for (query, _) in SEARCH_GROUPS {
        let disabled_search = client
            .send_request("tools/call", tool_call("search", json!({ "query": query })))
            .await
            .unwrap();
        assert_contains_no_optional_files(response_text(&disabled_search));
    }

    let disabled_overview = client
        .send_request("tools/call", tool_call("overview", json!({})))
        .await
        .unwrap();
    assert_contains_no_optional_files(response_text(&disabled_overview));

    let find = client
        .send_request(
            "tools/call",
            tool_call("find", json!({ "pattern": "**/*" })),
        )
        .await
        .unwrap();
    assert_contains_all_optional_files(response_text(&find));

    let grep = client
        .send_request(
            "tools/call",
            tool_call("grep", json!({ "pattern": OPTIONAL_TOKEN })),
        )
        .await
        .unwrap();
    assert_contains_all_optional_files(response_text(&grep));

    for (path, _) in OPTIONAL_FILES {
        let read = client
            .send_request(
                "tools/call",
                tool_call("read", json!({ "file_path": path })),
            )
            .await
            .unwrap();
        assert!(
            response_text(&read).contains(OPTIONAL_TOKEN)
                || response_text(&read).contains("GroupBoundaryToken"),
            "{path}: {}",
            response_text(&read)
        );
    }

    fs::write(temp.path().join(".codemap/config.toml"), config(true)).unwrap();
    for (query, path_prefix) in SEARCH_GROUPS {
        let enabled_search = client
            .send_tool_until("search", json!({ "query": query }), |text| {
                OPTIONAL_FILES
                    .iter()
                    .filter(|(path, _)| path.starts_with(path_prefix))
                    .all(|(path, _)| text.contains(path))
            })
            .await
            .unwrap();
        assert_contains_group_files(response_text(&enabled_search), path_prefix);
    }

    let enabled_overview = client
        .send_request("tools/call", tool_call("overview", json!({})))
        .await
        .unwrap();
    assert_contains_all_optional_files(response_text(&enabled_overview));

    for (query, path_prefix) in SEARCH_GROUPS {
        let enabled_cli_search = cli_stdout(&["search", query], temp.path());
        assert_contains_group_files(&enabled_cli_search, path_prefix);
    }
    for (path, _) in OPTIONAL_FILES {
        run_cli(&["codemap", "--path", path], temp.path()).success();
    }

    fs::write(temp.path().join(".codemap/config.toml"), config(false)).unwrap();
    for (query, _) in SEARCH_GROUPS {
        let disabled_again = client
            .send_tool_until("search", json!({ "query": query }), |text| {
                !text.contains("warming up")
                    && OPTIONAL_FILES.iter().all(|(path, _)| !text.contains(path))
            })
            .await
            .unwrap();
        assert_contains_no_optional_files(response_text(&disabled_again));
    }

    let disabled_overview_again = client
        .send_request("tools/call", tool_call("overview", json!({})))
        .await
        .unwrap();
    assert_contains_no_optional_files(response_text(&disabled_overview_again));
}

#[tokio::test]
async fn test_junk_files_require_include_ignored_for_find_and_grep() {
    let temp = fixture(false, true);
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let default_find = client
        .send_request(
            "tools/call",
            tool_call("find", json!({ "pattern": "**/*" })),
        )
        .await
        .unwrap();
    let default_grep = client
        .send_request(
            "tools/call",
            tool_call("grep", json!({ "pattern": JUNK_TOKEN })),
        )
        .await
        .unwrap();
    for (path, _) in JUNK_FILES {
        assert!(!response_text(&default_find).contains(path));
        assert!(!response_text(&default_grep).contains(path));
    }

    let bypass_find = client
        .send_request(
            "tools/call",
            tool_call(
                "find",
                json!({ "pattern": "**/*", "include_ignored": true }),
            ),
        )
        .await
        .unwrap();
    let bypass_grep = client
        .send_request(
            "tools/call",
            tool_call(
                "grep",
                json!({ "pattern": JUNK_TOKEN, "include_ignored": true }),
            ),
        )
        .await
        .unwrap();
    for (path, _) in JUNK_FILES {
        assert!(
            response_text(&bypass_find).contains(path),
            "missing {path}: {}",
            response_text(&bypass_find)
        );
        assert!(
            response_text(&bypass_grep).contains(path),
            "missing {path}: {}",
            response_text(&bypass_grep)
        );
    }
}
