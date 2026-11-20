# World packs

A pack is what a world contains: its maps, where a character may stand in each, how they connect, and
what a character is created with. `Vanilla` is the faithful base. A second pack is a folder beside it,
and the setting that names the pack is what selects it - `World.<Pack>` in `worldserver.json`, with
`Vanilla` the default.

| pack | is |
|---|---|
| `Vanilla/` | the base game's own values: the map catalogue, the doorway geometry, the creation screen's content. Every number in it is either the client's (it decides what a number means) or a measurement, and the document that records the source is `docs/` |

Nothing else lives at this level. A pack is a folder with a README and its own subfolders, and no file
here is shared between packs: a pack that inherits nothing is a pack whose numbers can be checked
against a capture without asking which layer overrode what.

The maps are not in these folders yet. They are rows of the world database today
(`data/sql/base/db_world/`), and a pack's `maps/` folder arrives with the stage that reads maps from
the pack rather than from SQL. Until then this folder holds the creation screen's content, which is the
part of a pack the world already reads from a file.
