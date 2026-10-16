# Attribution — related-terms PoC dataset

The round-1 dataset (schema `rt-dataset/1`, normalization `rtn-2`) was a restructured derivative of the sources below,
and the related-terms dataset `rtr1-930791c83bea6132` (schema `rt-related/1`) carries its relations. Changes made by
the builder: records were selected, lookup keys were normalized (lowercase, whitespace-collapsed, identifier-split
token keys), and relations were materialized into JSONL/SQLite. No upstream wording was rewritten; glosses are
copied verbatim. The pinned revisions are listed below and in `docs/related-terms/sources.md`. Copies must be
accompanied by the upstream license files named below; the local dataset folder does not include them.

No source endorses this dataset or the codemap-search project.

## Open English WordNet 2025 (`oewn-2025`)

- Source: https://github.com/globalwordnet/english-wordnet — release `2025-edition`, commit
  `dc343f2683279ecbb13fab4e2fd778d7b162d287`, asset `english-wordnet-2025-json.zip`
  (SHA-256 `7d749f6e2c39e6970e4997839dcf6e42fd281f3c2fae0171d2192bae8cfa4b51`).
- License: Creative Commons Attribution 4.0 International (CC BY 4.0), © The Open English WordNet Team.
- Upstream `LICENSE.md` states that the resource is derived from Princeton WordNet under the WordNet License, that
  attribution must be given to both Princeton WordNet and the Open English Wordnet team, and that the original
  Princeton WordNet license terms apply to the underlying data. The `WNDB_License.txt` notice from the same tag must
  accompany copies.
- Imported: synset members and sense keys, part of speech, lexicographer file, first definition, `domain_topic`,
  `similar`, and sense-level `antonym`.

## GitHub Explore (`github-explore-c8d7bc5`)

- Source: https://github.com/github/explore — commit `c8d7bc516928a4f17845b767884a80cc53affd57`, `topics/*/index.md`.
- License: "Content based on github.com/github/explore used under the CC-BY-4.0 license." GitHub trademarks are not
  licensed.
- Imported: topic slug, `display_name`, `aliases`, `related`, `short_description`; `wikipedia_url` is used only to
  select Wikidata entities.

## Wikidata (`wikidata-sel1`)

- Source: https://www.wikidata.org — entities selected from GitHub Explore `wikipedia_url` links plus three authoring
  diagnostic entities; each entity is pinned by its `lastrevid` (see `concepts.jsonl`).
- License: Wikidata structured data is available under CC0 1.0 (https://www.wikidata.org/wiki/Wikidata:Licensing).
- Imported: English label, English aliases, English description. No Wikipedia article text (CC BY-SA) is imported;
  English Wikipedia redirect titles were used only to resolve entity identity.

## abbreviations-in-code (`abbrcode-bd33b1c`)

- Source: https://github.com/abbrcode/abbreviations-in-code — commit `bd33b1c7bbfea159275c88d223482f5da9a01e58`,
  `data/abbrs/.json`.
- License: MIT License, Copyright (c) 2022 Gábor Kiss-Vámosi. The full notice is in the upstream `LICENCE` at the pinned commit.
- Imported: `word`, `abbr`, `degree`, `context`. Degrees are naming guidance, not probabilities.

## software-engineering-abbreviations-acronyms (`se-abbr-8e1d77e`)

- Source: https://github.com/wuyichen24/software-engineering-abbreviations-acronyms — commit
  `8e1d77e99b184106d50a180ba52cb3557273af6d`, `dictionary/**/*.md`.
- License: CC0 1.0 Universal.
- Imported: abbreviation from the entry file name, full name from the entry title, first overview line.
