"""Where the word-evaluation scripts find the PoC code, the pinned checkouts and their work directory.

Scripts and judging briefs live next to this module. Everything a run reads or writes (collected identifiers,
judging batches and results, summaries) lives under the work directory named by `WORD_EVAL_WORK_DIR`; nothing is
written next to the scripts. The work directory is read when a script needs it, so importing a module does not
require it.
"""

import os
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
EXPERIMENTS = SCRIPTS.parent
REPO = EXPERIMENTS.parents[3]
CHECKOUTS = REPO / ".codemap/poc/related-terms/workspaces/checkouts"
DATASETS = REPO / ".codemap/poc/related-terms/thesaurus-v1/datasets"
WORK_DIR_VARIABLE = "WORD_EVAL_WORK_DIR"


def work_dir() -> Path:
    """The work directory; exits with a message when `WORD_EVAL_WORK_DIR` is unset."""
    value = os.environ.get(WORK_DIR_VARIABLE)
    if not value:
        raise SystemExit(f"{WORK_DIR_VARIABLE} must name the word-evaluation work directory")
    return Path(value).expanduser().resolve()


def batches_dir() -> Path:
    """Judging inputs, results and briefs handed to agents (formerly /tmp/word-eval-*)."""
    return work_dir() / "batches"
