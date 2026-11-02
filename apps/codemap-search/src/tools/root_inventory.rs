//! The root scope inventory appended to the monorepo `initial_instructions` response: reads the
//! committed codemap snapshot the indexer publishes. Orchestration only.
//!
//! The dispatch arm (`crate::mcp`) calls `EngineSupervisor::ensure_alive`/`trigger_refresh`
//! before delegating here; this body only reads the committed snapshot through `engine`.

use crate::codemap::CodemapView;
use crate::index::EngineSupervisor;

/// Render the root inventory, or a warming/stopped notice before the first index exists.
pub(crate) fn render(engine: &EngineSupervisor) -> String {
    // Keep the readiness observed before this snapshot so a concurrent initial publish
    // cannot make an empty pre-index snapshot lose its warm-up notice.
    let is_warming = engine.is_warming();
    let published = engine.published_snapshot();
    let is_dead = engine.is_dead();
    let snapshot = published.codemap();
    if snapshot.is_empty() && (is_warming || is_dead) {
        return if is_dead {
            "Background indexer stopped before the codemap was built; restart the server. Use find/grep/read for live results."
        } else {
            "Codemap is warming up (initial background indexing in progress). Retry shortly, or use find/grep/read for live results."
        }
        .to_string();
    }
    published.workspace_catalog().root_view().unwrap_or_else(|| {
        crate::codemap::CodemapGenerator::generate_root_view(&snapshot).to_markdown()
    })
}
