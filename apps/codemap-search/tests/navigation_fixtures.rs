//! Navigation-query fixture tests.
//!
//! These fixtures pin two separate contracts:
//! - `navigation.scm` is part of the runtime extraction query and must produce stable
//!   `NavigationFile` calls/imports/local bindings.
//! - `tags.scm` is a compile/fixture validation gate only; it is intentionally not included
//!   in the runtime query concat.
//!
//! Regeneration:
//!
//! ```sh
//! UPDATE_NAVIGATION_FIXTURES=1 cargo test --manifest-path apps/codemap-search/Cargo.toml \
//!     --test navigation_fixtures
//! ```

use std::path::{Path, PathBuf};

use codemap_search::parser::{CodeExtractor, CodeRange, NavigationFile, TreeSitterExtractor};
use serde::Serialize;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

struct NavigationFixture {
    name: &'static str,
    source_file: &'static str,
    tags_query: &'static str,
    expected_navigation: &'static str,
    expected_tags: &'static str,
    required_calls: &'static [&'static str],
    required_receiver_calls: &'static [(&'static str, &'static str)],
    required_imports: &'static [&'static str],
    required_locals: &'static [&'static str],
}

const NO_CALLS: &[&str] = &[];
const NO_RECEIVER_CALLS: &[(&str, &str)] = &[];
const NO_IMPORTS: &[&str] = &[];
const NO_LOCALS: &[&str] = &[];

const FIXTURES: &[NavigationFixture] = &[
    NavigationFixture {
        name: "typescript",
        source_file: "typescript/basic.ts",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "typescript_nested",
        source_file: "typescript/nested.ts",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["mapOrder", "toDto", "Number", "persist", "createAudit"],
        required_receiver_calls: &[
            ("mapOrder", "this.mapper"),
            ("toDto", "this"),
            ("persist", "this.repo"),
            ("track", "audit"),
        ],
        required_imports: &["OrderRepository", "createAudit"],
        required_locals: &["dto", "audit", "total"],
    },
    NavigationFixture {
        name: "typescript_controller",
        source_file: "typescript/controller.ts",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parseOrder", "submit", "ok"],
        required_receiver_calls: &[("submit", "this.service"), ("ok", "Response")],
        required_imports: &["Response", "parseOrder"],
        required_locals: &["payload", "result"],
    },
    NavigationFixture {
        name: "typescript_policy",
        source_file: "typescript/policy.ts",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["filter", "check", "map", "keys"],
        required_receiver_calls: &[
            ("filter", "this.rules"),
            ("check", "rule"),
            ("map", "active"),
            ("keys", "Object"),
        ],
        required_imports: &["Rule"],
        required_locals: &["active", "names", "ruleName", "severity", "metadata"],
    },
    NavigationFixture {
        name: "typescript_worker",
        source_file: "typescript/worker.ts",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["pending", "next", "process", "ack", "warn"],
        required_receiver_calls: &[
            ("pending", "this.queue"),
            ("next", "this.queue"),
            ("process", "this.processor"),
            ("ack", "this.queue"),
            ("warn", "this.logger"),
        ],
        required_imports: &["Logger"],
        required_locals: &["queuedJob", "job", "receipt", "error"],
    },
    NavigationFixture {
        name: "javascript",
        source_file: "javascript/basic.js",
        tags_query: "queries/javascript/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: &["UserService", "save", "persist", "flush", "run"],
        required_receiver_calls: &[("save", "user"), ("flush", "Cache")],
        required_imports: &["Cache", "persist", "UserService"],
        required_locals: &["user"],
    },
    NavigationFixture {
        name: "javascript_nested",
        source_file: "javascript/nested.js",
        tags_query: "queries/javascript/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &[
            "mapOrder",
            "createAudit",
            "Number",
            "persist",
            "track",
            "toDto",
        ],
        required_receiver_calls: &[
            ("mapOrder", "this.mapper"),
            ("persist", "this.repo"),
            ("track", "audit"),
            ("toDto", "this"),
        ],
        required_imports: &["createAudit", "OrderRepository"],
        required_locals: &["dto", "audit", "total"],
    },
    NavigationFixture {
        name: "javascript_controller",
        source_file: "javascript/controller.js",
        tags_query: "queries/javascript/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parseOrder", "submit", "ok"],
        required_receiver_calls: &[("submit", "this.service"), ("ok", "Response")],
        required_imports: &["Response", "parseOrder"],
        required_locals: &["payload", "result"],
    },
    NavigationFixture {
        name: "javascript_policy",
        source_file: "javascript/policy.js",
        tags_query: "queries/javascript/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["filter", "check", "map", "keys"],
        required_receiver_calls: &[
            ("filter", "this.rules"),
            ("check", "rule"),
            ("map", "active"),
            ("keys", "Object"),
        ],
        required_imports: &["Rule"],
        required_locals: &["active", "names", "ruleName", "severity", "metadata"],
    },
    NavigationFixture {
        name: "javascript_worker",
        source_file: "javascript/worker.js",
        tags_query: "queries/javascript/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["pending", "warn", "next", "process", "ack"],
        required_receiver_calls: &[
            ("pending", "this.queue"),
            ("warn", "this.logger"),
            ("next", "this.queue"),
            ("process", "this.processor"),
            ("ack", "this.queue"),
        ],
        required_imports: &["Logger"],
        required_locals: &["queuedJob", "job", "receipt", "error"],
    },
    NavigationFixture {
        name: "tsx",
        source_file: "tsx/basic.tsx",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "tsx_nested",
        source_file: "tsx/nested.tsx",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["useMemo", "formatMoney", "map", "onSelect"],
        required_receiver_calls: &[("map", "items")],
        required_imports: &["useMemo", "formatMoney"],
        required_locals: &["rows"],
    },
    NavigationFixture {
        name: "tsx_form",
        source_file: "tsx/form.tsx",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.form.navigation.json",
        expected_tags: "expected.form.tags.json",
        required_calls: &["useState", "validate", "submit", "setErrors"],
        required_receiver_calls: &[("validate", "props.validator"), ("submit", "props.service")],
        required_imports: &["useState"],
        required_locals: &["errors", "nextErrors"],
    },
    NavigationFixture {
        name: "tsx_dashboard",
        source_file: "tsx/dashboard.tsx",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.dashboard.navigation.json",
        expected_tags: "expected.dashboard.tags.json",
        required_calls: &["useEffect", "load", "setOrders", "map"],
        required_receiver_calls: &[("load", "client"), ("map", "orders")],
        required_imports: &["useEffect", "useState"],
        required_locals: &["orders"],
    },
    NavigationFixture {
        name: "tsx_menu",
        source_file: "tsx/menu.tsx",
        tags_query: "queries/typescript/tags.scm",
        expected_navigation: "expected.menu.navigation.json",
        expected_tags: "expected.menu.tags.json",
        required_calls: &["useMemo", "filter", "map", "onChoose"],
        required_receiver_calls: &[("filter", "items"), ("map", "visible")],
        required_imports: &["useMemo"],
        required_locals: &["visible"],
    },
    NavigationFixture {
        name: "python",
        source_file: "python/basic.py",
        tags_query: "queries/python/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "python_nested",
        source_file: "python/nested.py",
        tags_query: "queries/python/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["validate", "reserve", "persist", "OrderDto", "sum"],
        required_receiver_calls: &[
            ("validate", "self.policy"),
            ("reserve", "self.inventory"),
            ("persist", "self.repo"),
        ],
        required_imports: &["dataclass", "Decimal"],
        required_locals: &["dto", "total"],
    },
    NavigationFixture {
        name: "python_handler",
        source_file: "python/handler.py",
        tags_query: "queries/python/tags.scm",
        expected_navigation: "expected.handler.navigation.json",
        expected_tags: "expected.handler.tags.json",
        required_calls: &["loads", "parse_order", "submit", "json"],
        required_receiver_calls: &[("submit", "service"), ("json", "response")],
        required_imports: &["json", "parse_order"],
        required_locals: &["payload", "result"],
    },
    NavigationFixture {
        name: "python_policy",
        source_file: "python/policy.py",
        tags_query: "queries/python/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["check", "all", "append"],
        required_receiver_calls: &[("check", "rule"), ("append", "failures")],
        required_imports: &["Protocol"],
        required_locals: &["failures", "passed"],
    },
    NavigationFixture {
        name: "python_worker",
        source_file: "python/worker.py",
        tags_query: "queries/python/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["next_job", "process", "ack", "warning"],
        required_receiver_calls: &[
            ("next_job", "queue"),
            ("process", "processor"),
            ("ack", "queue"),
            ("warning", "logger"),
        ],
        required_imports: &["logging"],
        required_locals: &["job", "receipt"],
    },
    NavigationFixture {
        name: "go",
        source_file: "go/basic.go",
        tags_query: "queries/go/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "go_nested",
        source_file: "go/nested.go",
        tags_query: "queries/go/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["MapOrder", "Reserve", "Save", "Trace"],
        required_receiver_calls: &[
            ("MapOrder", "s.mapper"),
            ("Reserve", "s.inventory"),
            ("Save", "s.repo"),
        ],
        required_imports: &["context"],
        required_locals: &["dto", "audit"],
    },
    NavigationFixture {
        name: "go_handler",
        source_file: "go/handler.go",
        tags_query: "queries/go/tags.scm",
        expected_navigation: "expected.handler.navigation.json",
        expected_tags: "expected.handler.tags.json",
        required_calls: &["NewDecoder", "Decode", "Submit", "Encode"],
        required_receiver_calls: &[
            ("Decode", "decoder"),
            ("Submit", "h.service"),
            ("Encode", "json.NewEncoder(w)"),
        ],
        required_imports: &["json", "http"],
        required_locals: &["decoder", "request", "receipt"],
    },
    NavigationFixture {
        name: "go_policy",
        source_file: "go/policy.go",
        tags_query: "queries/go/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["Check", "append", "len"],
        required_receiver_calls: &[("Check", "rule")],
        required_imports: &["errors"],
        required_locals: &["failures"],
    },
    NavigationFixture {
        name: "go_worker",
        source_file: "go/worker.go",
        tags_query: "queries/go/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["Next", "Process", "Ack", "Warn"],
        required_receiver_calls: &[
            ("Next", "w.queue"),
            ("Process", "w.processor"),
            ("Ack", "w.queue"),
            ("Warn", "w.logger"),
        ],
        required_imports: &["context"],
        required_locals: &["job", "receipt"],
    },
    NavigationFixture {
        name: "rust",
        source_file: "rust/basic.rs",
        tags_query: "queries/rust/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "rust_nested",
        source_file: "rust/nested.rs",
        tags_query: "queries/rust/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["map", "reserve", "save", "audit_event"],
        required_receiver_calls: &[
            ("map", "self.mapper"),
            ("reserve", "self.inventory"),
            ("save", "self.repo"),
        ],
        required_imports: &["OrderRepo", "*"],
        required_locals: &["dto", "audit"],
    },
    NavigationFixture {
        name: "rust_handler",
        source_file: "rust/handler.rs",
        tags_query: "queries/rust/tags.scm",
        expected_navigation: "expected.handler.navigation.json",
        expected_tags: "expected.handler.tags.json",
        required_calls: &["parse_order", "submit", "ok"],
        required_receiver_calls: &[("submit", "self.service"), ("ok", "Response")],
        required_imports: &["Response"],
        required_locals: &["payload", "receipt"],
    },
    NavigationFixture {
        name: "rust_policy",
        source_file: "rust/policy.rs",
        tags_query: "queries/rust/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["iter", "filter", "collect", "is_empty", "insert"],
        required_receiver_calls: &[
            ("iter", "self.rules"),
            ("filter", "self.rules.iter()"),
            ("is_empty", "failures"),
            ("insert", "outcomes"),
        ],
        required_imports: &["HashMap"],
        required_locals: &["failures", "outcomes", "failure"],
    },
    NavigationFixture {
        name: "rust_worker",
        source_file: "rust/worker.rs",
        tags_query: "queries/rust/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["next", "retry_hint", "process", "ack", "warn"],
        required_receiver_calls: &[
            ("next", "self.queue"),
            ("retry_hint", "self.queue"),
            ("process", "self.processor"),
            ("ack", "self.queue"),
        ],
        required_imports: &["warn"],
        required_locals: &["job", "retry", "receipt"],
    },
    NavigationFixture {
        name: "java",
        source_file: "java/basic.java",
        tags_query: "queries/java/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "java_nested",
        source_file: "java/nested.java",
        tags_query: "queries/java/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["map", "reserve", "save", "audit"],
        required_receiver_calls: &[
            ("map", "mapper"),
            ("reserve", "inventory"),
            ("save", "repository"),
        ],
        required_imports: &["Service", "List"],
        required_locals: &["dto", "event"],
    },
    NavigationFixture {
        name: "java_controller",
        source_file: "java/controller.java",
        tags_query: "queries/java/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parse", "submit", "ok"],
        required_receiver_calls: &[
            ("parse", "parser"),
            ("submit", "service"),
            ("ok", "ResponseEntity"),
        ],
        required_imports: &["PostMapping", "ResponseEntity"],
        required_locals: &["request", "receipt"],
    },
    NavigationFixture {
        name: "java_policy",
        source_file: "java/policy.java",
        tags_query: "queries/java/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["stream", "filter", "toList", "isEmpty"],
        required_receiver_calls: &[("stream", "rules"), ("isEmpty", "failures")],
        required_imports: &["List"],
        required_locals: &["failures"],
    },
    NavigationFixture {
        name: "java_worker",
        source_file: "java/worker.java",
        tags_query: "queries/java/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["next", "process", "ack", "warn"],
        required_receiver_calls: &[
            ("next", "queue"),
            ("process", "processor"),
            ("ack", "queue"),
            ("warn", "logger"),
        ],
        required_imports: &["Scheduled"],
        required_locals: &["job", "receipt"],
    },
    NavigationFixture {
        name: "kotlin",
        source_file: "kotlin/basic.kt",
        tags_query: "queries/kotlin/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "kotlin_nested",
        source_file: "kotlin/nested.kt",
        tags_query: "queries/kotlin/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["map", "reserve", "save", "audit"],
        required_receiver_calls: &[
            ("map", "mapper"),
            ("reserve", "inventory"),
            ("save", "repository"),
        ],
        required_imports: &["OrderMapper"],
        required_locals: &["dto", "event"],
    },
    NavigationFixture {
        name: "kotlin_controller",
        source_file: "kotlin/controller.kt",
        tags_query: "queries/kotlin/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parse", "submit", "ok"],
        required_receiver_calls: &[
            ("parse", "parser"),
            ("submit", "service"),
            ("ok", "ResponseEntity"),
        ],
        required_imports: &["PostMapping"],
        required_locals: &["request", "receipt"],
    },
    NavigationFixture {
        name: "kotlin_policy",
        source_file: "kotlin/policy.kt",
        tags_query: "queries/kotlin/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["filter", "isEmpty", "map"],
        required_receiver_calls: &[
            ("filter", "rules"),
            ("isEmpty", "failures"),
            ("map", "failures"),
        ],
        required_imports: &["Rule"],
        required_locals: &["failures", "names"],
    },
    NavigationFixture {
        name: "kotlin_worker",
        source_file: "kotlin/worker.kt",
        tags_query: "queries/kotlin/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["next", "process", "ack", "warn"],
        required_receiver_calls: &[
            ("next", "queue"),
            ("process", "processor"),
            ("ack", "queue"),
            ("warn", "logger"),
        ],
        required_imports: &["Scheduled"],
        required_locals: &["job", "receipt"],
    },
    NavigationFixture {
        name: "c",
        source_file: "c/basic.c",
        tags_query: "queries/c/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "c_nested",
        source_file: "c/nested.c",
        tags_query: "queries/c/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["map_order", "reserve_inventory", "repo_save", "TRACE_ORDER"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["order"],
        required_locals: &["dto", "saved"],
    },
    NavigationFixture {
        name: "c_handler",
        source_file: "c/handler.c",
        tags_query: "queries/c/tags.scm",
        expected_navigation: "expected.handler.navigation.json",
        expected_tags: "expected.handler.tags.json",
        required_calls: &["parse_order", "checkout_submit", "write_response"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["handler"],
        required_locals: &["request", "receipt"],
    },
    NavigationFixture {
        name: "c_policy",
        source_file: "c/policy.c",
        tags_query: "queries/c/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["rule_check", "append_failure", "failure_count"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["policy"],
        required_locals: &["failures", "passed"],
    },
    NavigationFixture {
        name: "c_worker",
        source_file: "c/worker.c",
        tags_query: "queries/c/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["queue_next", "process_job", "queue_ack", "log_warn"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["worker"],
        required_locals: &["job", "receipt"],
    },
    NavigationFixture {
        name: "cpp",
        source_file: "cpp/basic.cpp",
        tags_query: "queries/cpp/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "cpp_nested",
        source_file: "cpp/nested.cpp",
        tags_query: "queries/cpp/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["map", "reserve", "save", "audit"],
        required_receiver_calls: &[
            ("map", "mapper_"),
            ("reserve", "inventory_"),
            ("save", "repo_"),
        ],
        required_imports: &["order"],
        required_locals: &["dto", "audit"],
    },
    NavigationFixture {
        name: "cpp_controller",
        source_file: "cpp/controller.cpp",
        tags_query: "queries/cpp/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parse", "submit", "ok"],
        required_receiver_calls: &[
            ("parse", "parser_"),
            ("submit", "service_"),
            ("ok", "Response"),
        ],
        required_imports: &["controller"],
        required_locals: &["request", "receipt"],
    },
    NavigationFixture {
        name: "cpp_policy",
        source_file: "cpp/policy.cpp",
        tags_query: "queries/cpp/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["check", "push_back", "name", "empty"],
        required_receiver_calls: &[
            ("check", "rule"),
            ("push_back", "failures"),
            ("name", "rule"),
            ("empty", "failures"),
        ],
        required_imports: &["policy"],
        required_locals: &["failures"],
    },
    NavigationFixture {
        name: "cpp_worker",
        source_file: "cpp/worker.cpp",
        tags_query: "queries/cpp/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["next", "process", "ack", "warn"],
        required_receiver_calls: &[
            ("next", "queue_"),
            ("process", "processor_"),
            ("ack", "queue_"),
            ("warn", "logger_"),
        ],
        required_imports: &["worker"],
        required_locals: &["job", "receipt"],
    },
    NavigationFixture {
        name: "csharp",
        source_file: "csharp/basic.cs",
        tags_query: "queries/csharp/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "csharp_nested",
        source_file: "csharp/nested.cs",
        tags_query: "queries/csharp/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["Map", "Save"],
        required_receiver_calls: &[("Map", "mapper"), ("Save", "repo")],
        required_imports: &["Repo"],
        required_locals: &["dto"],
    },
    NavigationFixture {
        name: "csharp_controller",
        source_file: "csharp/controller.cs",
        tags_query: "queries/csharp/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["Parse", "Submit"],
        required_receiver_calls: &[("Parse", "Request"), ("Submit", "service")],
        required_imports: &["Request"],
        required_locals: &["request"],
    },
    NavigationFixture {
        name: "csharp_policy",
        source_file: "csharp/policy.cs",
        tags_query: "queries/csharp/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["Check"],
        required_receiver_calls: &[("Check", "rule")],
        required_imports: &["Rule"],
        required_locals: &["passed"],
    },
    NavigationFixture {
        name: "csharp_worker",
        source_file: "csharp/worker.cs",
        tags_query: "queries/csharp/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["Next", "Process", "Ack"],
        required_receiver_calls: &[
            ("Next", "queue"),
            ("Process", "processor"),
            ("Ack", "queue"),
        ],
        required_imports: &["Job"],
        required_locals: &["job"],
    },
    NavigationFixture {
        name: "php",
        source_file: "php/basic.php",
        tags_query: "queries/php/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "php_nested",
        source_file: "php/nested.php",
        tags_query: "queries/php/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["map", "save"],
        required_receiver_calls: &[("map", "$mapper"), ("save", "$repo")],
        required_imports: &["Repo"],
        required_locals: &["$mapper", "$repo"],
    },
    NavigationFixture {
        name: "php_controller",
        source_file: "php/controller.php",
        tags_query: "queries/php/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parse", "submit"],
        required_receiver_calls: &[("parse", "Request"), ("submit", "$service")],
        required_imports: &["Request"],
        required_locals: &["$service"],
    },
    NavigationFixture {
        name: "php_policy",
        source_file: "php/policy.php",
        tags_query: "queries/php/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["check"],
        required_receiver_calls: &[("check", "$rule")],
        required_imports: &["Rule"],
        required_locals: &["$rule"],
    },
    NavigationFixture {
        name: "php_worker",
        source_file: "php/worker.php",
        tags_query: "queries/php/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["next", "process", "ack"],
        required_receiver_calls: &[
            ("next", "$queue"),
            ("process", "$processor"),
            ("ack", "$queue"),
        ],
        required_imports: &["Job"],
        required_locals: &["$queue", "$processor"],
    },
    NavigationFixture {
        name: "ruby",
        source_file: "ruby/basic.rb",
        tags_query: "queries/ruby/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "ruby_nested",
        source_file: "ruby/nested.rb",
        tags_query: "queries/ruby/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["map", "save"],
        required_receiver_calls: &[("map", "mapper"), ("save", "repo")],
        required_imports: &["repo"],
        required_locals: &["mapper", "repo"],
    },
    NavigationFixture {
        name: "ruby_controller",
        source_file: "ruby/controller.rb",
        tags_query: "queries/ruby/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parse", "submit"],
        required_receiver_calls: &[("parse", "Request"), ("submit", "service")],
        required_imports: &["request"],
        required_locals: &["service"],
    },
    NavigationFixture {
        name: "ruby_policy",
        source_file: "ruby/policy.rb",
        tags_query: "queries/ruby/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["check"],
        required_receiver_calls: &[("check", "rule")],
        required_imports: &["rule"],
        required_locals: &["rule"],
    },
    NavigationFixture {
        name: "ruby_worker",
        source_file: "ruby/worker.rb",
        tags_query: "queries/ruby/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["next", "process", "ack"],
        required_receiver_calls: &[
            ("next", "queue"),
            ("process", "processor"),
            ("ack", "queue"),
        ],
        required_imports: &["job"],
        required_locals: &["queue", "processor"],
    },
    NavigationFixture {
        name: "lua",
        source_file: "lua/basic.lua",
        tags_query: "queries/lua/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "lua_nested",
        source_file: "lua/nested.lua",
        tags_query: "queries/lua/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &["map", "save"],
        required_receiver_calls: &[("map", "mapper"), ("save", "repo")],
        required_imports: &["repo"],
        required_locals: &["mapper", "repo"],
    },
    NavigationFixture {
        name: "lua_controller",
        source_file: "lua/controller.lua",
        tags_query: "queries/lua/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parse", "submit"],
        required_receiver_calls: &[("parse", "request"), ("submit", "service")],
        required_imports: &["request"],
        required_locals: &["service"],
    },
    NavigationFixture {
        name: "lua_policy",
        source_file: "lua/policy.lua",
        tags_query: "queries/lua/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["check"],
        required_receiver_calls: &[("check", "rule")],
        required_imports: &["rule"],
        required_locals: &["rule"],
    },
    NavigationFixture {
        name: "lua_worker",
        source_file: "lua/worker.lua",
        tags_query: "queries/lua/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &["next", "process", "ack"],
        required_receiver_calls: &[
            ("next", "queue"),
            ("process", "processor"),
            ("ack", "queue"),
        ],
        required_imports: &["job"],
        required_locals: &["queue", "processor"],
    },
    NavigationFixture {
        name: "asm",
        source_file: "asm/basic.s",
        tags_query: "queries/asm/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: &["init_runtime", "flush_runtime"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["runtime"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "asm_nested",
        source_file: "asm/nested.s",
        tags_query: "queries/asm/tags.scm",
        expected_navigation: "expected.nested.navigation.json",
        expected_tags: "expected.nested.tags.json",
        required_calls: &[
            "map_order",
            "reserve_inventory",
            "persist_order",
            "audit_finish",
        ],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["audit"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "asm_controller",
        source_file: "asm/controller.s",
        tags_query: "queries/asm/tags.scm",
        expected_navigation: "expected.controller.navigation.json",
        expected_tags: "expected.controller.tags.json",
        required_calls: &["parse_order", "submit_order", "response_ok"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["http"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "asm_policy",
        source_file: "asm/policy.s",
        tags_query: "queries/asm/tags.scm",
        expected_navigation: "expected.policy.navigation.json",
        expected_tags: "expected.policy.tags.json",
        required_calls: &["filter_rules", "check_rule", "map_names", "object_keys"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["rules"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "asm_worker",
        source_file: "asm/worker.s",
        tags_query: "queries/asm/tags.scm",
        expected_navigation: "expected.worker.navigation.json",
        expected_tags: "expected.worker.tags.json",
        required_calls: &[
            "queue_pending",
            "queue_next",
            "process_job",
            "queue_ack",
            "logger_warn",
        ],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["queue"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "swift",
        source_file: "swift/basic.swift",
        tags_query: "queries/swift/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: &["targetSwift"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["Foundation"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "dart",
        source_file: "dart/basic.dart",
        tags_query: "queries/dart/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: &["targetDart"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["helper"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "scala",
        source_file: "scala/basic.scala",
        tags_query: "queries/scala/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: &["targetScala"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["Helper"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "scala_script",
        source_file: "scala/script.sc",
        tags_query: "queries/scala/tags.scm",
        expected_navigation: "expected.script.navigation.json",
        expected_tags: "expected.script.tags.json",
        required_calls: &["println"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "groovy",
        source_file: "groovy/basic.groovy",
        tags_query: "queries/groovy/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: &["targetGroovy"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["Helper"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "gradle",
        source_file: "groovy/build.gradle",
        tags_query: "queries/groovy/tags.scm",
        expected_navigation: "expected.gradle.navigation.json",
        expected_tags: "expected.gradle.tags.json",
        required_calls: NO_CALLS,
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "powershell",
        source_file: "powershell/basic.ps1",
        tags_query: "queries/powershell/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: &["Import-Module", "TargetPowerShell"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: &["Demo.Helper"],
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "powershell_module",
        source_file: "powershell/module.psm1",
        tags_query: "queries/powershell/tags.scm",
        expected_navigation: "expected.module.navigation.json",
        expected_tags: "expected.module.tags.json",
        required_calls: &["Start-Worker", "Export-ModuleMember"],
        required_receiver_calls: NO_RECEIVER_CALLS,
        required_imports: NO_IMPORTS,
        required_locals: NO_LOCALS,
    },
    NavigationFixture {
        name: "nix",
        source_file: "nix/basic.nix",
        tags_query: "queries/nix/tags.scm",
        expected_navigation: "expected.navigation.json",
        expected_tags: "expected.tags.json",
        required_calls: &[
            "transform",
            "callPackage",
            "helper",
            "helperArg",
            "caller",
            "import",
        ],
        required_receiver_calls: &[("transform", "lib"), ("import", "builtins")],
        required_imports: &["./package.nix", "./module.nix"],
        required_locals: &["lib", "callPackage", "value", "helperArg"],
    },
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TagCapture {
    capture: String,
    text: String,
    range: CodeRange,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/navigation")
}

fn manifest_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn language_for_source(source_file: &str) -> Language {
    match Path::new(source_file)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
    {
        "ts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "js" => tree_sitter_javascript::LANGUAGE.into(),
        "py" => tree_sitter_python::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "rs" => tree_sitter_rust::LANGUAGE.into(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        "cs" => tree_sitter_c_sharp::LANGUAGE.into(),
        "php" => tree_sitter_php::LANGUAGE_PHP.into(),
        "rb" => tree_sitter_ruby::LANGUAGE.into(),
        "lua" => tree_sitter_lua::LANGUAGE.into(),
        "kt" => {
            unsafe extern "C" {
                fn tree_sitter_kotlin() -> *const ();
            }
            // Use the same bundled native grammar as the product extractor.
            unsafe { tree_sitter_language::LanguageFn::from_raw(tree_sitter_kotlin) }.into()
        }
        "c" => tree_sitter_c::LANGUAGE.into(),
        "cpp" => tree_sitter_cpp::LANGUAGE.into(),
        "s" => tree_sitter_asm::LANGUAGE.into(),
        "swift" => tree_sitter_swift::LANGUAGE.into(),
        "dart" => tree_sitter_dart::LANGUAGE.into(),
        "scala" | "sc" => tree_sitter_scala::LANGUAGE.into(),
        "groovy" | "gradle" => {
            unsafe extern "C" {
                fn tree_sitter_groovy() -> *const ();
            }
            // Built by the package alongside its other native grammar functions.
            unsafe { tree_sitter_language::LanguageFn::from_raw(tree_sitter_groovy) }.into()
        }
        "ps1" | "psm1" => tree_sitter_powershell::LANGUAGE.into(),
        "nix" => tree_sitter_nix::LANGUAGE.into(),
        ext => panic!("unsupported navigation fixture extension: {ext}"),
    }
}

fn range_for_node(node: tree_sitter::Node) -> CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

fn pretty_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("serialize navigation fixture")
}

fn first_diff(expected: &str, actual: &str) -> String {
    for (line_number, (expected_line, actual_line)) in
        expected.lines().zip(actual.lines()).enumerate()
    {
        if expected_line != actual_line {
            return format!(
                "first difference at line {}:\n  golden: {expected_line}\n  actual: {actual_line}",
                line_number + 1
            );
        }
    }
    format!(
        "golden has {} lines, actual has {} lines (no differing line within the shared prefix)",
        expected.lines().count(),
        actual.lines().count()
    )
}

fn compare_or_update(path: &Path, actual: &str) {
    if std::env::var_os("UPDATE_NAVIGATION_FIXTURES").is_some() {
        std::fs::write(path, format!("{actual}\n"))
            .unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
        return;
    }

    let expected = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read expected fixture {}: {err}", path.display()));
    let expected = expected.strip_suffix('\n').unwrap_or(&expected);
    assert!(
        expected == actual,
        "navigation fixture drift at {}\n{}",
        path.display(),
        first_diff(expected, actual)
    );
}

fn extract_navigation(fixture: &NavigationFixture, source: &str) -> Option<NavigationFile> {
    let extractor = TreeSitterExtractor::new();
    let extracted = extractor
        .extract(source, fixture.source_file)
        .unwrap_or_else(|err| panic!("extract {}: {err}", fixture.source_file));
    extracted.navigation
}

fn assert_required_navigation(fixture: &NavigationFixture, navigation: &NavigationFile) {
    for required_call in fixture.required_calls {
        assert!(
            navigation
                .calls
                .iter()
                .any(|call| call.name == *required_call),
            "{} should capture call `{}`; actual calls: {:?}",
            fixture.name,
            required_call,
            navigation
                .calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>()
        );
    }
    for (required_name, required_receiver) in fixture.required_receiver_calls {
        assert!(
            navigation.calls.iter().any(|call| {
                call.name == *required_name && call.receiver.as_deref() == Some(*required_receiver)
            }),
            "{} should capture receiver call `{}.{}`; actual calls: {:?}",
            fixture.name,
            required_receiver,
            required_name,
            navigation
                .calls
                .iter()
                .map(|call| (call.receiver.as_deref().unwrap_or(""), call.name.as_str()))
                .collect::<Vec<_>>()
        );
    }
    for required_import in fixture.required_imports {
        assert!(
            navigation
                .imports
                .iter()
                .any(|entry| entry.local_name == *required_import),
            "{} should capture import `{}`; actual imports: {:?}",
            fixture.name,
            required_import,
            navigation
                .imports
                .iter()
                .map(|entry| entry.local_name.as_str())
                .collect::<Vec<_>>()
        );
    }
    for required_local in fixture.required_locals {
        assert!(
            navigation
                .local_bindings
                .iter()
                .any(|binding| binding.name == *required_local),
            "{} should capture local `{}`; actual locals: {:?}",
            fixture.name,
            required_local,
            navigation
                .local_bindings
                .iter()
                .map(|binding| binding.name.as_str())
                .collect::<Vec<_>>()
        );
    }
}

fn actual_tags_json(fixture: &NavigationFixture, source: &str) -> String {
    let language = language_for_source(fixture.source_file);
    let query_text = std::fs::read_to_string(manifest_path(fixture.tags_query))
        .unwrap_or_else(|err| panic!("read {}: {err}", fixture.tags_query));
    let query = Query::new(&language, &query_text)
        .unwrap_or_else(|err| panic!("compile {}: {err}", fixture.tags_query));
    let source_bytes = source.as_bytes();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .unwrap_or_else(|err| panic!("set language for {}: {err}", fixture.source_file));
    let tree = parser
        .parse(source, None)
        .unwrap_or_else(|| panic!("parse {}", fixture.source_file));
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
    let mut captures = Vec::new();
    while let Some(query_match) = matches.next() {
        for capture in query_match.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            let text = capture
                .node
                .utf8_text(source_bytes)
                .unwrap_or("")
                .trim()
                .to_string();
            captures.push(TagCapture {
                capture: capture_name.to_string(),
                text,
                range: range_for_node(capture.node),
            });
        }
    }
    pretty_json(&captures)
}

fn check_fixture(fixture: &NavigationFixture) {
    let source_path = fixtures_dir().join(fixture.source_file);
    assert!(
        source_path.exists(),
        "missing navigation fixture source for {} at {}",
        fixture.name,
        source_path.display()
    );
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|err| panic!("read {}: {err}", source_path.display()));
    let fixture_dir = source_path
        .parent()
        .unwrap_or_else(|| panic!("fixture without parent: {}", fixture.source_file));
    let navigation = extract_navigation(fixture, &source)
        .unwrap_or_else(|| panic!("{} should produce navigation output", fixture.name));
    assert_required_navigation(fixture, &navigation);
    compare_or_update(
        &fixture_dir.join(fixture.expected_navigation),
        &pretty_json(&Some(navigation)),
    );
    compare_or_update(
        &fixture_dir.join(fixture.expected_tags),
        &actual_tags_json(fixture, &source),
    );
}

#[test]
fn navigation_fixtures_match_expected_json() {
    for fixture in FIXTURES {
        check_fixture(fixture);
    }
}
