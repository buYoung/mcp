//! Provider routing. The configured model ID alone selects the HTTP API, so a credential is
//! only ever sent to the API that serves that model:
//! - a bare Jev version such as `jev-1.14.0` posts to TypeSafe's System One API
//!   ([`super::ENDPOINT`]);
//! - an `author/slug` ID such as `openai/gpt-6-luna-decisions` posts to OpenRouter's Decisions
//!   API ([`super::OPENROUTER_ENDPOINT`]). The runtime never calls OpenRouter's chat or other
//!   APIs. That endpoint serves decision models only: OpenRouter answers a chat or unknown
//!   model with HTTP 400 ("Model … does not exist") before any model runs.
//!
//! Both APIs accept the same request body and return the same answer shapes. OpenRouter adds
//! `id`, `provider` and `usage.cost`, which validation ignores, and reports its canonical model
//! slug (see [`Provider::is_requested_model`]).
//!
//! OpenRouter decision models known to accept this runtime's object state, with at least the
//! 64k context the operating budgets assume (OpenRouter catalog
//! `GET /api/v1/models?output_modalities=decisions`, checked 2026-10-24):
//! - `openai/gpt-6-luna-decisions`
//! - `typesafe/jev-1.14`, `typesafe/jev-1.13`
//! - `perplexity/pplx-decider-v1.2`, `perplexity/pplx-decider-flash-v1.2`
//! - `nace-ai/drex-max`, `nace-ai/drex-v2.0`, `nace-ai/drex-v1.5`
//! - `respan/span-02`
//! - `upstage/solar-decide`, `upstage/solar-decide-flash`
//! - `cloudflare/clef`, `cloudflare/clef-omni`
//! - `liquid/d1`
//! - `inception/mercury-decide`
//!
//! `scaledown/classify-1` is a decision model but rejects object state fields other than its
//! own document schema, and smaller-context models cannot hold the operating budgets.

/// The HTTP API that serves one configured model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    TypeSafe,
    OpenRouter,
}

impl Provider {
    /// OpenRouter model IDs carry an author prefix (`author/slug`); TypeSafe IDs never do.
    pub fn for_model(model: &str) -> Self {
        if model.contains('/') {
            Self::OpenRouter
        } else {
            Self::TypeSafe
        }
    }

    pub fn endpoint(self) -> &'static str {
        match self {
            Self::TypeSafe => super::ENDPOINT,
            Self::OpenRouter => super::OPENROUTER_ENDPOINT,
        }
    }

    /// Reject a model that cannot be pinned before any request is built. An OpenRouter alias
    /// (`~author/slug`) answers as whichever version it currently points to.
    pub(crate) fn validate_model(self, model: &str) -> Result<(), String> {
        if self == Self::OpenRouter && model.starts_with('~') {
            return Err(
                "OpenRouter alias models (~author/slug) are not accepted; configure a pinned model ID"
                    .into(),
            );
        }
        Ok(())
    }

    /// Whether `actual`, the model a response reports, is the configured `requested` model.
    /// TypeSafe echoes the exact version. OpenRouter reports its canonical slug: the requested
    /// ID without a `:variant` suffix, optionally followed by a `-YYYYMMDD` release date
    /// (`typesafe/jev-1.14` answers as `typesafe/jev-1.14-20261014`).
    pub(crate) fn is_requested_model(self, requested: &str, actual: &str) -> bool {
        if actual == requested {
            return true;
        }
        if self != Self::OpenRouter {
            return false;
        }
        let base = requested
            .split_once(':')
            .map_or(requested, |(base, _variant)| base);
        actual == base
            || actual
                .strip_prefix(base)
                .and_then(|rest| rest.strip_prefix('-'))
                .is_some_and(|date| {
                    date.len() == 8 && date.bytes().all(|byte| byte.is_ascii_digit())
                })
    }
}
