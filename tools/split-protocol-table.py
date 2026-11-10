"""Split the merged protocol table into the two programs that own its halves.

The table came into this tree as one file because ASURA kept one. AzerothCore keeps each program's
opcodes in the program that serves that connection, and this tree follows the reference: the world's
rows belong to `game/src/Server/Protocol/`, the login's rows to `authserver/src/Protocol/`. What stays
shared is the *mechanism* - the field types and the walk that frames with them - because both programs
need it and the auth program does not link the game library.

Input:  shared/src/Packets/Tables.rs  (782 rows, both families, one file)
Output: game/src/Server/Protocol/Tables.rs       (the 682 world rows, and their names)
        authserver/src/Protocol/Tables.rs       (the 100 login rows, and their names)

Run once. It is the record of the move; nothing regenerates the tables afterwards.
"""

import io
import os
import re
import sys

SOURCE = "shared/src/Packets/Tables.rs"
GAME_OUT = "game/src/Server/Protocol/Tables.rs"
AUTH_OUT = "authserver/src/Protocol/Tables.rs"


def read(path):
    with io.open(path, encoding="utf-8", newline="") as handle:
        return handle.read()


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with io.open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)


def block(text, start_regex, end_marker="\n}"):
    """The text from the line matching `start_regex` through its closing brace."""
    match = re.search(start_regex, text)
    if not match:
        return None
    start = match.start()
    end = text.index(end_marker, start) + len(end_marker)
    return text[start:end]


def main():
    text = read(SOURCE)

    # 1. The rows, one family each.
    rows = [line for line in text.splitlines() if line.startswith("    Packet {")]
    game_rows = [line for line in rows if "conn: Conn::Game," in line]
    auth_rows = [line for line in rows if "conn: Conn::Auth," in line]

    # 2. The name blocks.
    game_names = block(text, r"pub mod game \{")
    auth_names = block(text, r"pub mod auth \{")
    if not (game_names and auth_names):
        print("could not find the two name modules")
        return 1

    # 3. Each program's own table: its rows, its lookup, its names.
    def header(family, count, other):
        return (
            "//! The %s family of this client's protocol: %d rows, ported from ASURA's table.\n"
            "//!\n"
            "//! One row per header: the wire value, the index the reference addresses its format array\n"
            "//! with, the declared size, the name and the field layout a decoder walks. The walker itself\n"
            "//! is `shared::packets::Frame` - the mechanism both connections use - and the other family's\n"
            "//! rows live in the program that serves %s.\n"
            "//!\n"
            "//! Source: `ASURA/crates/asura-protocol/src/packets.rs`, split by\n"
            "//! `tools/split-protocol-table.py`. `docs/port/02-wire.md` says how a stream is split with it.\n"
            "\n"
            "use shared::packets::tables::{Conn, Field, FieldKind, Packet, Status};\n"
            "\n"
            "/// The family's rows: `%d`, sorted by client-to-server first, then header.\n"
            "#[rustfmt::skip]\n"
            "static ROWS: &[Packet] = &[\n"
            % (family, count, other, count)
        )

    def footer(family):
        return (
            "];\n"
            "\n"
            "/// The row for one header in one direction, or `None` if this family has no such message.\n"
            "///\n"
            "/// A binary search over the family's rows: framing asks this for every message.\n"
            "pub fn lookup(header: u16, from_client: bool) -> Option<&'static Packet> {\n"
            "    ROWS.binary_search_by(|row| {\n"
            "        (row.from_client, row.header).cmp(&(from_client, header)).reverse()\n"
            "    })\n"
            "    .ok()\n"
            "    .map(|index| &ROWS[index])\n"
            "}\n"
            "\n"
            "/// Every row of the family.\n"
            "pub fn rows() -> impl Iterator<Item = &'static Packet> {\n"
            "    ROWS.iter()\n"
            "}\n"
            "\n"
            "%s\n" % ("__NAMES__",)
        )

    def emit(path, family, count, rows_, names, other):
        body = header(family, count, other) + "\n".join(rows_) + "\n" + footer(family)
        body = body.replace("__NAMES__", names)
        write(path, body)
        print("wrote", path, "with", len(rows_), "rows")

    emit(GAME_OUT, "game", len(game_rows), game_rows, game_names, "the login connection")
    emit(AUTH_OUT, "login", len(auth_rows), auth_rows, auth_names, "the world")

    # 4. The sorted order each lookup binary-searches.
    for path, rows_ in ((GAME_OUT, game_rows), (AUTH_OUT, auth_rows)):
        keys = []
        for line in rows_:
            from_client = "true" if "from_client: true" in line else "false"
            header = int(re.search(r"header: (0x[0-9A-Fa-f]+),", line).group(1), 16)
            keys.append((from_client, header))
        ordered = sorted(keys, key=lambda key: (0 if key[0] == "true" else 1, key[1]))
        if ordered != keys:
            print("NOT SORTED for", path, "- the binary search would be wrong")
        else:
            print("sorted:", path)

    return 0


if __name__ == "__main__":
    sys.exit(main())
