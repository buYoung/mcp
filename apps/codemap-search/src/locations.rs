//! Source locations in an optional, explicit enclosing-file context.

pub(crate) fn display(path: &str, line: usize, current_file: Option<&str>) -> String {
    if current_file == Some(path) {
        format!("L{line}")
    } else {
        format!("{path}:{line}")
    }
}
