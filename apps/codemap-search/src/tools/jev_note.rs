//! One-line Jev outcome notes appended to tool text. Applied, bypassed, and fallback
//! outcomes stay distinguishable, and usage and timing are reported separately.
use crate::jev::{Timing, Usage};

pub(crate) fn applied(
    tool: &str,
    detail: &str,
    usage: Usage,
    timing: Timing,
    request_count: usize,
) -> String {
    format!(
        "\n\n[jev {tool}: applied · {detail} · {}]",
        accounting(usage, timing, request_count)
    )
}

pub(crate) fn fallback(
    tool: &str,
    reason: &str,
    usage: Usage,
    timing: Timing,
    request_count: usize,
) -> String {
    format!(
        "\n\n[jev {tool}: fallback ({reason}) · original output preserved · {}]",
        accounting(usage, timing, request_count)
    )
}

pub(crate) fn bypassed(tool: &str, reason: &str) -> String {
    format!("\n\n[jev {tool}: bypassed ({reason}) · original output unchanged]")
}

fn accounting(usage: Usage, timing: Timing, request_count: usize) -> String {
    format!(
        "requests={request_count} · input_tokens={} · output_tokens={} · elapsed_ms={} · http_ms={}",
        usage.input_tokens,
        usage.output_tokens,
        timing.elapsed.as_millis(),
        timing.http_elapsed.as_millis()
    )
}
