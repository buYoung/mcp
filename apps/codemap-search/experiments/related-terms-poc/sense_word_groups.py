"""Word groups used to measure and widen the sense dataset: fields per word and the words of the recent steps."""

from collections import defaultdict
import json
from pathlib import Path

from term_canonical import word_key

DATASETS = Path(__file__).resolve().parent / "datasets"
# The omission (25df), binding (08b1) and new-concept (5fba) steps, each adding words to one proposed field.
RECENT_STEPS = (("rts3-25df1cf41d0d8247/entries.json", "personas"),
                ("rts3-08b1780cc2880c26/corrections.json", "input_bindings"),
                ("rts3-5fba9ba33d7c7c7f/entries.json", "personas"))


def recent_words():
    """Word keys the recent steps added or bound."""
    return {word_key(row["word"]) for path, key in RECENT_STEPS for row in json.loads((DATASETS / path).read_text())[key]}


def word_fields(index):
    """Fields per word key as lookup reads them (`sense_lookup.SenseIndex.senses_of`): own senses and `input_senses`."""
    fields = defaultdict(set)
    for key in set(index.senses) | set(index.input_personas):
        fields[key] = {sense.field for sense in index.senses_of(key)}
    return fields
