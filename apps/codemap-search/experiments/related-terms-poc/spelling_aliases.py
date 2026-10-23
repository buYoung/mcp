"""Explicit code-evidenced spellings; never replace or fuzzy-correct the original query.

Each pair was independently reviewed against the fixed 12-repository corpus.
These are whole lexical-unit spellings (using the existing case/separator key),
not substring rewrites of arbitrary identifiers. Only one mapping step is used.
"""
from term_canonical import canonical_surface, word_key

CONFIRMED_SPELLINGS = {
    "backhannel": "backchannel",  # Keycloak CibaPolicy.tsx: BACKHANNEL options for BackchannelTokenDeliveryMode.
    "polices": "policies",        # Keycloak setupPolices builds Policy objects; original policing senses remain.
    "acquring": "acquiring",      # Redis module_gil_acquring is read into the acquiring variable.
    "cirds": "cidrs",             # Gin TrustedCIRDs tests compare expectedTrustedCIDRs/trustedCIDRs.
    "mata": "meta",               # Redis functionExtractLibMetaData accepts functionsLibMataData.
    "asterick": "asterisk",       # Redis special_asterick stores the literal *.
    "certficate": "certificate",  # Keycloak getCertficateResource returns a certificate resource.
    "comitted": "committed",      # Keycloak inComittedTransaction registers commitImpl/onCommit callbacks.
    "conut": "count",             # Tokio *_conut tests assert strong_count/weak_count.
    "replicy": "policy",          # Redis restore_replicy has type restorePolicy and reads restore_policy_str.
    "uncontented": "uncontended", # Tokio uncontented benchmark group calls uncontended functions.
}


def spelling_variants(query):
    """Original spelling first, followed by its single confirmed alternative, if any."""
    original = canonical_surface(query)
    alternative = CONFIRMED_SPELLINGS.get(word_key(original))
    return (original, alternative) if alternative else (original,)


def spelling_queries(queries):
    """Queries needed by the existing matcher, including explicit spelling alternatives."""
    return sorted({variant for query in queries for variant in spelling_variants(query)})
