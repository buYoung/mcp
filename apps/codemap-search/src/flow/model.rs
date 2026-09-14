use crate::parser::{CodeRange, ImportEntry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) type NodeId = usize;
pub(crate) type BindingId = usize;
pub(crate) type FunctionId = usize;
pub(crate) type ClassId = usize;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlowFile {
    pub digest: String,
    pub units: Vec<FlowUnit>,
    pub omissions: Vec<FlowIssue>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlowUnit {
    pub language: String,
    pub namespace: String,
    pub has_modified_map_builtin: bool,
    pub nodes: Vec<Expression>,
    pub bindings: Vec<Binding>,
    pub functions: Vec<FunctionSummary>,
    pub classes: Vec<Class>,
    pub exports: BTreeMap<String, Export>,
    pub imports: Vec<ImportEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Export {
    Local(String),
    Foreign { source: String, name: String },
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Binding {
    pub name: String,
    pub range: CodeRange,
    pub scope: CodeRange,
    pub kind: BindingKind,
    pub initializer: Option<NodeId>,
    pub is_mutated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum BindingKind {
    Local,
    Parameter { function: FunctionId, index: usize },
    Function(FunctionId),
    Class(ClassId),
    Import(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FunctionSummary {
    pub name: String,
    pub range: CodeRange,
    pub parameters: Vec<BindingId>,
    pub body: Vec<Statement>,
    pub owner: Option<ClassId>,
    pub is_available: bool,
    pub is_method: bool,
    pub is_static: bool,
    pub has_lexical_receiver: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Class {
    pub name: String,
    pub range: CodeRange,
    pub constructor: Option<FunctionId>,
    pub methods: BTreeMap<String, FunctionId>,
    pub fields: Vec<(String, NodeId)>,
    pub is_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Expression {
    pub range: CodeRange,
    pub function: FunctionId,
    pub kind: ExpressionKind,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum Constant {
    String(String),
    Number(String),
    Boolean(bool),
    Null,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ExpressionKind {
    Read {
        name: String,
        binding: Option<BindingId>,
    },
    Literal(Constant),
    Function(FunctionId),
    Object {
        class: Option<String>,
        fields: Vec<(String, NodeId)>,
    },
    Field {
        object: NodeId,
        key: NodeId,
    },
    Call {
        callee: NodeId,
        arguments: Vec<NodeId>,
    },
    Construct {
        callee: NodeId,
        arguments: Vec<NodeId>,
    },
    Unknown {
        reason: String,
        inputs: Vec<NodeId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Statement {
    pub range: CodeRange,
    pub kind: StatementKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StatementKind {
    Bind {
        binding: BindingId,
        value: NodeId,
    },
    Assign {
        target: NodeId,
        value: NodeId,
    },
    Return(NodeId),
    Evaluate(NodeId),
    Barrier {
        reason: String,
        expressions: Vec<NodeId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlowIssue {
    pub range: CodeRange,
    pub reason: String,
}

impl FlowFile {
    pub(crate) fn apply_path_constraints(&mut self, path: &str, source: &str) {
        // Go build tags and platform selection require a compiler target model.
        // Retain declarations, but do not compose an inactive/unknown body.
        let go_suffix = std::path::Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| {
                stem.rsplit('_').take(2).any(|part| {
                    matches!(
                        part,
                        "linux"
                            | "windows"
                            | "darwin"
                            | "android"
                            | "ios"
                            | "freebsd"
                            | "openbsd"
                            | "netbsd"
                            | "dragonfly"
                            | "solaris"
                            | "illumos"
                            | "aix"
                            | "plan9"
                            | "js"
                            | "wasip1"
                            | "amd64"
                            | "arm64"
                            | "386"
                            | "arm"
                            | "ppc64"
                            | "ppc64le"
                            | "mips"
                            | "mipsle"
                            | "mips64"
                            | "mips64le"
                            | "riscv64"
                            | "s390x"
                            | "wasm"
                            | "loong64"
                    )
                })
            });
        let go_tags = source.lines().take(64).any(|line| {
            line.trim_start().starts_with("//go:build")
                || line.trim_start().starts_with("// +build")
        });
        for unit in &mut self.units {
            if unit.language == "go" && (go_suffix || go_tags) {
                for function in &mut unit.functions {
                    function.is_available = false;
                }
                if self.omissions.len() < 16 {
                    self.omissions.push(FlowIssue {
                        range: unit.functions[0].range.clone(),
                        reason:
                            "Go build/platform conditions are unresolved for function summaries"
                                .into(),
                    });
                }
            }
        }
    }
    pub(crate) fn merge(&mut self, other: Self) {
        for unit in other.units {
            let nodes = self
                .units
                .iter()
                .map(|unit| unit.nodes.len())
                .sum::<usize>();
            let bindings = self
                .units
                .iter()
                .map(|unit| unit.bindings.len())
                .sum::<usize>();
            let functions = self
                .units
                .iter()
                .map(|unit| unit.functions.len())
                .sum::<usize>();
            if nodes + unit.nodes.len() > super::NODES_PER_FILE
                || bindings + unit.bindings.len() > super::NODES_PER_FILE
                || functions + unit.functions.len() > super::FUNCTIONS_PER_FILE
            {
                if self.omissions.len() < 16 {
                    self.omissions.push(FlowIssue {
                        range: unit.functions[0].range.clone(),
                        reason: "composite file summary budget exceeded".into(),
                    });
                }
            } else {
                self.units.push(unit);
            }
        }
        self.omissions.extend(
            other
                .omissions
                .into_iter()
                .take(16usize.saturating_sub(self.omissions.len())),
        );
    }
}
