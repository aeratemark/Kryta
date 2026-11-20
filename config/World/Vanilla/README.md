# Vanilla

The faithful base pack: the client's own vocabulary and the values a faithful world runs on.

```text
Vanilla/
  creation/
    inventory.json    the bags and the items a creation screen is given, before a profession is chosen
    equipment.json    what a new character wears, one set per (campaign, profession)
    worn_slots.json   where a worn item sits: the cell inside the equipped bag, and its position in the
                      agent's nine-dword visual list
```

- `creation/` is one subject: everything the character-creation screen is built from and everything a
  new character leaves with. Its three files are read together at start-up (`InventoryFile`,
  `EquipmentFile` and `EquippedSlotsFile` in `worldserver.json`), and the world logs what it read.
- `inventory.json` and `equipment.json` are imported, not written by hand: `tools/import-creation-inventory.py`
  regenerates them from ASURA's generated equipment table, which came from the reference's own
  `GetDefaultEquipments`. `worn_slots.json` is written by hand, because its order is a measurement and
  its two numberings must agree with a capture (`docs/game-rules.md` E234).
- A file that would describe something the world does not read yet is not here. `character_spawn.json`
  (the outpost each campaign's new character starts in, `docs/game-rules.md` E232) arrives with the
  stage that reads it, not before.

Every file carries a `note` and an `information` block saying what a row means and where the numbers
came from, so a reader does not have to open the code to know what they are looking at.
