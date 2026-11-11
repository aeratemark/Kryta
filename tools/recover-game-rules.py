"""Recover the owner's game-rules ledger into this tree, as ASCII.

The ledger lived at `docs/36-game-rules-ledger.md` in ASURA's history and was removed when that repo's
documents were reworked. It is the descriptions this project ports against: the owner's asserted rules
and the evidenced facts behind them, by subject.

Read from `git show f7c2f31:docs/36-game-rules-ledger.md`; written to `docs/game-rules.md` with a short
provenance note above the original text. The body is otherwise the owner's document, unchanged, with the
few non-ASCII characters transliterated because every file in this tree is ASCII.
"""

import io
import sys

SOURCE = sys.argv[1] if len(sys.argv) > 1 else None
TARGET = "docs/game-rules.md"

TRANSLITERATIONS = {
    "\u2014": " - ",
    "\u2013": "-",
    "\u2019": "'",
    "\u2018": "'",
    "\u201c": '"',
    "\u201d": '"',
    "\u2026": "...",
    "\u00a0": " ",
    "\u2192": "->",
    "\u2190": "<-",
    "\u00b7": "-",
}

PROVENANCE = '''# The game rules: the owner's assertions and the evidence for them

**Where this came from.** ASURA's `docs/36-game-rules-ledger.md`, written by the owner on 2026-11-02 and
kept through several revisions; it was removed when that repository's documents were reworked. Recovered
from `git show f7c2f31:docs/36-game-rules-ledger.md` on 2026-11-28 and copied here, because it is the
description of how the game works that this port is measured against - not a plan, and not another
project's opinion.

**How to read it.** The status vocabulary is the document's own and still applies: a rule the owner
asserted is law until he corrects it, an evidenced fact names its witness, a contradicted rule keeps both
rows, and a suspect rule may not be depended on without raising it. Where this file and a capture or the
client disagree, the client wins and the disagreement is reported (`AGENTS.md`, rule 3).

**What it is not.** It is not a port order, not an architecture, and not a list of features to build in
sequence. `docs/port/06-world-plan.md` maps each subject here to the noun that owns it.

---

'''

with io.open(SOURCE, encoding="utf-8-sig") as handle:
    body = handle.read()

for source, target in TRANSLITERATIONS.items():
    body = body.replace(source, target)

# The recovered document's own first line is its title; keep it as a section heading under the note.
lines = body.splitlines()
if lines and lines[0].startswith("# "):
    lines[0] = "# " + lines[0][2:]
body = "\n".join(lines)

with io.open(TARGET, "w", encoding="ascii", newline="\n") as handle:
    handle.write(PROVENANCE)
    handle.write(body)
    if not body.endswith("\n"):
        handle.write("\n")

print("wrote", TARGET)
