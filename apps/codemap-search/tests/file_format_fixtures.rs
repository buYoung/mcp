use codemap_search::parser::{CodeExtractor, TreeSitterExtractor};

fn extract(path: &str, source: &str) -> codemap_search::parser::ExtractedFile {
    TreeSitterExtractor::new().extract(source, path).unwrap()
}

fn has_symbol(file: &codemap_search::parser::ExtractedFile, name: &str) -> bool {
    file.symbols.iter().any(|symbol| symbol.name == name)
}

#[test]
fn test_registry_routes_only_tree_sitter_formats() {
    assert!(has_symbol(
        &extract("config.json", r#"{"server": 1}"#),
        "server"
    ));
    assert!(has_symbol(&extract("page.html", "<main></main>"), "main"));
    assert!(has_symbol(&extract("site.scss", ".card {}"), ".card"));
    assert!(has_symbol(&extract("site.less", ".card {}"), ".card"));
    assert!(has_symbol(
        &extract("deploy.bash", "function deploy { :; }"),
        "deploy"
    ));
    assert!(has_symbol(
        &extract("deploy.zsh", "function deploy { :; }"),
        "deploy"
    ));
    assert!(has_symbol(&extract("Dockerfile", "ARG VERSION"), "VERSION"));
    assert!(has_symbol(
        &extract("Makefile", "build: input\n\t@echo ok"),
        "build"
    ));
    assert!(has_symbol(
        &extract("CMakeLists.txt", "add_executable(app main.cpp)"),
        "app"
    ));
    assert!(has_symbol(
        &extract("BUILD", "cc_library(name = \"x\")"),
        "x"
    ));
}

#[test]
fn test_checked_priority_aliases_have_grammar_backed_symbols() {
    for (path, source, symbol) in [
        ("config.json", r#"{"key": 1}"#, "key"),
        ("config.jsonc", "// comment\n{ \"key\": 1 }", "key"),
        ("config.toml", "key = 1", "key"),
        ("config.yaml", "key: 1", "key"),
        ("config.yml", "key: 1", "key"),
        ("page.html", "<main />", "main"),
        ("page.htm", "<main />", "main"),
        ("page.xml", "<main />", "main"),
        ("schema.xsd", "<main />", "main"),
        ("page.xsl", "<main />", "main"),
        ("page.xslt", "<main />", "main"),
        ("Info.plist", "<main />", "main"),
        ("app.csproj", "<main />", "main"),
        ("app.props", "<main />", "main"),
        ("app.targets", "<main />", "main"),
        ("site.css", ".card {}", ".card"),
        ("site.scss", "$color: red;\n.card {}", ".card"),
        ("site.less", "@color: red;\n.card {}", ".card"),
        ("deploy.sh", "run() { :; }", "run"),
        ("deploy.bash", "run() { :; }", "run"),
        ("deploy.zsh", "run() { :; }", "run"),
        ("main.hcl", "variable \"region\" {}", "region"),
        ("main.tf", "terraform {}", "terraform"),
        ("api.proto", "message Request {}", "Request"),
        ("schema.graphql", "type Query { id: ID! }", "Query"),
        ("schema.gql", "type Query { id: ID! }", "Query"),
        ("Dockerfile", "ARG VERSION\nFROM rust AS build", "VERSION"),
        ("rules.mk", "build: input", "build"),
        ("build.cmake", "add_library(core core.cpp)", "core"),
        ("rules.bzl", "def helper():\n    pass", "helper"),
        ("BUILD", "cc_library(name = \"core\")", "core"),
    ] {
        assert!(
            has_symbol(&extract(path, source), symbol),
            "missing {symbol} from {path}"
        );
    }
    assert!(
        extract("values.tfvars", "region = \"kr\"")
            .navigation
            .is_some(),
        "tfvars uses the HCL AST route"
    );
}

#[test]
fn priority_one_ast_keys_have_nested_paths_and_comment_isolation() {
    let jsonc = extract(
        "config.jsonc",
        "// comment\n{ \"server\": { \"port\": 5000 } }",
    );
    assert!(has_symbol(&jsonc, "server.port"));
    let toml = extract(
        "config.toml",
        "[server] # comment\nport = 5000\n# fake = 1\n",
    );
    assert!(has_symbol(&toml, "server.port"));
    assert!(!has_symbol(&toml, "# fake"));
    let yaml = extract("config.yaml", "server:\n  port: 5000\n");
    assert!(has_symbol(&yaml, "server.port"));
}

#[test]
fn priority_two_ast_symbols_exclude_unverified_components() {
    let html = extract("page.html", "<img id=\"hero\" class=\"asset image\" />");
    assert!(has_symbol(&html, "img") && has_symbol(&html, "hero") && has_symbol(&html, "asset"));
    let xml = extract(
        "page.xml",
        "<root><child id=\"node\" class=\"leaf\"/></root>",
    );
    assert!(
        has_symbol(&xml, "root")
            && has_symbol(&xml, "child")
            && has_symbol(&xml, "node")
            && has_symbol(&xml, "leaf")
    );
    let css = extract("site.css", ".card { --brand-color: red; } @keyframes pulse { from { opacity: 0 } } @media (width > 10px) {}");
    assert!(
        has_symbol(&css, ".card") && has_symbol(&css, "--brand-color") && has_symbol(&css, "pulse")
    );
    assert!(
        !has_symbol(&css, "width"),
        "media feature names are not declarations"
    );
    let component = extract(
        "Widget.astro",
        "---\nconst fake = \"<main>\"\n---\n<main />",
    );
    assert!(
        !has_symbol(&component, "main"),
        "template markup must stay unstructured"
    );
}

#[test]
fn priority_three_ast_symbols_and_imports_preserve_no_call_policy() {
    let shell = extract("deploy.sh", "function deploy { :; }\nREGION=kr\nsource ./shared.sh\nsource \"$PLUGIN_ROOT/common.sh\"\n. \"$(resolve_lib)\"");
    assert!(has_symbol(&shell, "deploy") && has_symbol(&shell, "REGION"));
    let navigation = shell.navigation.unwrap();
    assert_eq!(navigation.imports[0].source.as_deref(), Some("./shared.sh"));
    assert_eq!(
        navigation.imports.len(),
        1,
        "dynamic source operands are not static imports"
    );
    assert!(navigation.calls.is_empty() && navigation.references.is_empty());
}

#[test]
fn priority_four_ast_relationships_exclude_comments_and_docker_structure() {
    let hcl = extract("main.tf", "terraform { required_version = \">= 1.8\" }\nmodule \"network\" { source = \"./modules/network\" }\n/* module.fake.id */\nvalue = module.real.id");
    assert!(has_symbol(&hcl, "terraform"));
    let navigation = hcl.navigation.unwrap();
    assert_eq!(
        navigation.imports[0].source.as_deref(),
        Some("./modules/network")
    );
    assert!(navigation
        .references
        .iter()
        .any(|reference| reference.name == "module.real.id"));
    assert!(!navigation
        .references
        .iter()
        .any(|reference| reference.name.contains("fake")));
    let proto = extract(
        "api.proto",
        "message User {}\nmessage Request { User owner = 1; }",
    );
    assert!(proto
        .navigation
        .unwrap()
        .references
        .iter()
        .any(|reference| reference.name == "User"));
    let graph = extract("schema.graphql", "schema { query: Query }\ntype User { id: ID! }\nfragment UserFields on User { id }\nquery Find { user { ...UserFields } }");
    assert!(has_symbol(&graph, "schema"));
    assert!(graph
        .navigation
        .unwrap()
        .references
        .iter()
        .any(|reference| reference.name == "UserFields"));
    let docker = extract("Dockerfile", "ARG VERSION\nFROM rust AS build");
    assert!(has_symbol(&docker, "VERSION"));
    assert!(has_symbol(&docker, "build"));
    assert_eq!(
        docker.navigation.unwrap().imports[0].source.as_deref(),
        Some("rust")
    );
}

#[test]
fn added_tree_sitter_formats_emit_structural_symbols_and_dependencies() {
    let zsh = extract("deploy.zsh", "deploy() { :; }\nsource ./shared.zsh");
    assert!(has_symbol(&zsh, "deploy"));
    assert_eq!(
        zsh.navigation.unwrap().imports[0].source.as_deref(),
        Some("./shared.zsh")
    );

    let make = extract("Makefile", "build: input\n\t@echo ok\nVERSION := 1");
    assert!(has_symbol(&make, "build"));
    assert!(has_symbol(&make, "VERSION"));
    assert!(make
        .navigation
        .unwrap()
        .references
        .iter()
        .any(|reference| reference.name == "input"));

    let cmake = extract(
        "CMakeLists.txt",
        "add_library(core core.cpp)\ntarget_link_libraries(app core)\ninclude(shared.cmake)",
    );
    assert!(has_symbol(&cmake, "core"));
    let cmake_navigation = cmake.navigation.unwrap();
    assert!(cmake_navigation
        .references
        .iter()
        .any(|reference| reference.name == "core"));
    assert_eq!(
        cmake_navigation.imports[0].source.as_deref(),
        Some("shared.cmake")
    );

    let starlark = extract(
        "BUILD",
        "load(\"//tools:defs.bzl\", \"rule\")\ncc_library(name = \"core\", deps = [\"//base\"])",
    );
    assert!(has_symbol(&starlark, "core"));
    let starlark_navigation = starlark.navigation.unwrap();
    assert_eq!(
        starlark_navigation.imports[0].source.as_deref(),
        Some("//tools:defs.bzl")
    );
    assert!(starlark_navigation
        .references
        .iter()
        .any(|reference| reference.name == "//base"));
}

#[test]
fn broad_tree_sitter_grammar_coverage_keeps_overview_symbols_complete() {
    for (path, source, expected) in [
        (
            "config.json",
            r#"{"services":[{"name":"api","port":8080}]}"#,
            &["services", "services.name", "services.port"][..],
        ),
        (
            "config.toml",
            "root = { nested = { leaf = 1 } }",
            &["root", "root.nested", "root.nested.leaf"],
        ),
        (
            "config.yaml",
            "services: [{name: api, port: 8080}]",
            &["services", "services.name", "services.port"],
        ),
        (
            "page.html",
            r#"<main id="app" class="shell wide responsive"></main>"#,
            &["main", "app", "shell", "wide", "responsive"],
        ),
        (
            "page.xml",
            r#"<root class="primary secondary"><child id="leaf"/></root>"#,
            &["root", "primary", "secondary", "child", "leaf"],
        ),
        (
            "site.css",
            "a[href]:hover::before, .card { --gap: 1rem; } @keyframes pulse {}",
            &[
                "a",
                "a[href]",
                "a[href]:hover",
                "a[href]:hover::before",
                ".card",
                "--gap",
                "pulse",
            ],
        ),
        (
            "site.scss",
            "$gap: 1rem; @mixin surface($color) { color: $color; } @function spacing($n) { @return $n * $gap; } .card:hover {}",
            &["$gap", "surface", "spacing", ".card", ".card:hover"],
        ),
        (
            "site.less",
            "@gap: 1rem; .surface(@color) { color: @color; } .card:hover {}",
            &["@gap", ".surface", ".card", ".card:hover"],
        ),
        (
            "deploy.sh",
            "function deploy { :; }\nREGION=kr\nCACHE=(a b)\nsource ./shared.sh",
            &["deploy", "REGION", "CACHE"],
        ),
        (
            "deploy.zsh",
            "function prepare deploy { :; }\nREGION=kr\nsource ./shared.zsh",
            &["prepare", "deploy", "REGION"],
        ),
        (
            "main.tf",
            "region = \"kr\"\nresource \"aws_s3_bucket\" \"assets\" { bucket = var.bucket\n lifecycle { prevent_destroy = true } }",
            &["region", "aws_s3_bucket.assets", "bucket", "lifecycle", "prevent_destroy"],
        ),
        (
            "Dockerfile",
            "ARG VERSION\nENV REGION=kr CACHE=on\nLABEL owner=platform tier=api\nFROM rust:${VERSION} AS build",
            &["VERSION", "REGION", "CACHE", "owner", "tier", "build"],
        ),
        (
            "api.proto",
            "package demo.v1; message User { string name = 1; map<string, int32> scores = 2; oneof identity { string email = 3; } } enum State { STATE_UNKNOWN = 0; } service Api { rpc Get(User) returns (User); } extend User { string nickname = 100; }",
            &["demo.v1", "User", "name", "scores", "identity", "email", "State", "STATE_UNKNOWN", "Api", "Get", "nickname"],
        ),
        (
            "schema.graphql",
            "type Query { user(id: ID!): User } extend type Query { other: String } input Filter { term: String } enum Role { ADMIN USER } query GetUser { user(id: \"1\") { id } }",
            &["Query", "user", "id", "other", "Filter", "term", "Role", "ADMIN", "USER", "GetUser"],
        ),
        (
            "Makefile",
            "VPATH = src\n.RECIPEPREFIX := >\nREV != git rev-parse HEAD\ndefine banner\nhello\nendef\nall package: compile assets\n>echo ok",
            &["VPATH", ".RECIPEPREFIX", "REV", "banner", "all", "package"],
        ),
        (
            "CMakeLists.txt",
            "project(App)\nadd_library(core core.cpp)\nadd_test(NAME unit COMMAND app)\nfind_package(OpenSSL REQUIRED)\nFetchContent_Declare(fmt URL https://example.invalid/fmt.tgz)",
            &["App", "core", "unit", "OpenSSL", "fmt"],
        ),
        (
            "BUILD",
            "first, second = (1, 2)\ndef helper():\n    pass\ncc_library(name = \"core\", srcs = [\"core.cc\"], deps = [\"//base\"], data = [\"//runtime\"], tools = [\"//tools:gen\"])",
            &["first", "second", "helper", "core"],
        ),
    ] {
        let extracted = extract(path, source);
        for symbol in expected {
            assert!(
                has_symbol(&extracted, symbol),
                "missing {symbol} from broad {path} grammar coverage: {:#?}",
                extracted.symbols
            );
        }
    }
}

#[test]
fn malformed_priority_grammars_preserve_recoverable_ast_boundaries() {
    for (path, source, stable, rejected) in [
        (
            "broken.json",
            r#"{ "stable": 1, "ghost": }"#,
            "stable",
            "ghost",
        ),
        (
            "broken.jsonc",
            "// valid comment\n{ \"stable\": 1, \"ghost\": }",
            "stable",
            "ghost",
        ),
        ("broken.toml", "stable = 1\nghost =", "stable", "ghost"),
        ("broken.yaml", "stable: 1\nghost: [}", "stable", "ghost"),
        ("broken.html", "<stable></stable><ghost", "stable", "ghost"),
        (
            "broken.xml",
            "<root><stable/><ghost</root>",
            "stable",
            "ghost",
        ),
        ("broken.css", ".stable {}\n.ghost {", ".stable", ".ghost"),
        ("broken.scss", ".stable {}\n.ghost {", ".stable", ".ghost"),
        ("broken.less", ".stable {}\n.ghost {", ".stable", ".ghost"),
        ("broken.sh", "stable() { :; }\nghost() {", "stable", "ghost"),
        (
            "broken.zsh",
            "stable() { :; }\nghost() {",
            "stable",
            "ghost",
        ),
        (
            "broken.tf",
            "variable \"stable\" {}\nresource \"aws_s3_bucket\" \"ghost\" { value = module.fake.id",
            "stable",
            "ghost",
        ),
        (
            "broken.proto",
            "message Stable {}\nmessage Ghost { string name =",
            "Stable",
            "Ghost",
        ),
        (
            "broken.graphql",
            "type Stable { id: ID }\ntype Ghost { id: ID !! }",
            "Stable",
            "Ghost",
        ),
        (
            "broken.cmake",
            "add_library(stable a.c)\nadd_library(ghost",
            "stable",
            "ghost",
        ),
        ("broken.bzl", "stable = 1\nghost =", "stable", "ghost"),
    ] {
        let file = extract(path, source);
        assert!(
            has_symbol(&file, stable),
            "error recovery removed the valid sibling {stable} in {path}: {:#?}",
            file.symbols
        );
        assert!(
            !has_symbol(&file, rejected),
            "error recovery exposed {rejected} as a declaration in {path}: {:#?}",
            file.symbols
        );
        assert!(
            file.navigation
                .as_ref()
                .is_none_or(|navigation| navigation.references.iter().all(|reference| {
                    !reference.name.contains("fake") && reference.name != rejected
                })),
            "error recovery exposed a malformed relationship in {path}"
        );
    }
}
