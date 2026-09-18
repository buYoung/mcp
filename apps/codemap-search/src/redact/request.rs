//! Request-local activation of output redaction.
use std::cell::Cell;

thread_local! {
    static IS_MCP_RESPONSE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) struct RequestGuard(bool);

pub(crate) fn begin_request() -> RequestGuard {
    RequestGuard(IS_MCP_RESPONSE.replace(true))
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        IS_MCP_RESPONSE.set(self.0);
    }
}

pub(crate) fn is_enabled() -> bool {
    IS_MCP_RESPONSE.get() && crate::config::get().is_redact_enabled
}
