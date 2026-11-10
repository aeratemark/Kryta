# 0008 - The administration tool is a program of its own, outside the two servers

Status: accepted, taken because nothing in this tree could change game data except `psql` and a text
editor. The owner may overrule it; if he does, this record is corrected rather than extended.

## The question

Every piece of game data is now a row or a content file: accounts and their EULA version, characters and
their settings, the world list, the maps and the creation sets. A person needs to change them - create an
account, make a client accept the EULA again, correct a character's level, look at what a set contains.
Where does that live?

The reference has no such program. What it has is a worldserver console and a SOAP surface for in-game
operations, and everything else through the database by other means. Its `authserver` answers clients and
nothing more.

## The decision

A third binary, `tools/manage/` (cargo package `kryta-manage`), serving one HTML page over HTTP on
loopback. It links `common`, `database`, `game` and `shared` - the same libraries the servers link - and
reaches the data through the nouns' own repositories (`game/accounts`, `game/characters`). Neither server
program knows it exists, and it serves no game protocol.

## Why

1. **It is not a server.** Nothing about it is a client conversation, and putting operator power behind
   the listener a client talks to is how a login surface becomes an admin hole. The portal keeps its four
   routes exactly as the client expects them.
2. **The data access moves into the noun, not the tool.** `DatabaseAccounts` and `DatabaseCharacters` are
   library code, so the tool, the auth program and the world program ask one implementation what an
   account or a character is. A second copy of the SQL in the tool would drift from the schemas it
   describes.
3. **A page, because the state is spread out.** Two databases and four content files, with a live account
   on one side and a client on the other; a wrong `UPDATE` typed from memory is not a workflow.
4. **Loopback and a token.** The tool binds `127.0.0.1` and allows this machine by default; a `Token` in
   its settings makes it demand one. It has no authorisation model beyond that, which is why it is not
   reachable from anywhere else.

## What it costs

- A third cargo member, and the rule that keeps it honest: it may read and write data, never behaviour.
  A page that changes what the server *does* belongs in the server, behind a setting.
- One more settings file (`tools/manage/manage.json`), on the same JSON reader as the two programs.
- The EULA version becomes a column (`data/sql/base/db_auth/account_eula.sql`), because a flag an operator
  toggles is state, and state is a row.

## What it does not decide

Where a spawn or an instance record lives: the world schema does not exist yet, and its pages arrive with
it. Nor whether the same interface later hosts in-game operations (kick, teleport): those act on a running
world, which is the worldserver's business, not a page's.
