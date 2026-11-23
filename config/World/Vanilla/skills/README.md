# Skill scripts

One Lua file per skill, named by its `skills.json` row's `script`. What a skill costs, how long it
casts, its recharge, profession, attribute and `scale`/`bonus`/`duration` ranges stay in `skills.json`;
the script decides what the skill does (`docs/decisions/0016-skills-are-lua-scripts.md`).

Read once at start-up. Restart the world after editing a script.

## Shape

```lua
return {
    on_use = function(cast)
        cast:heal{ to = "target", amount = "scale", divine_favor = true }
    end,
}
```

`on_use` runs when the skill completes (after its activation, before the slot recharges).

## What `cast` holds (read-only)

| field | |
|---|---|
| `cast.skill`, `cast.name` | the skill id and name |
| `cast.rank` | the caster's rank in the skill's attribute |
| `cast.scale`, `cast.bonus`, `cast.duration` | the row's ranges, already read at `cast.rank` |
| `cast.aoe` | the row's area value |
| `cast.caster`, `cast.target` | `agent`, `alive`, `health`, `max_health`, `energy`, `max_energy`, `level`, `x`, `y`, `hexed`, `enchanted`, `attacking`, `casting`, `conditions` (a set: `cast.target.conditions.bleeding`), `effects` (skill ids) |

Changing these fields does nothing: the engine owns the state.

## What a script can queue

`cast:<word>{ fields }` queues one operation; `op.<word>{ fields }` builds one to nest (`on_hit`,
`pulse`, `hit`, `then`) or to pass to `cast:run(...)`. The words and their fields are ASURA's
`skills.json` effect operations, read by `game/Skills/Catalog.rs` (`read_op`):

`attack`, `damage`, `heal`, `life_steal`, `condition`, `knockdown`, `interrupt`, `effect`, `field`,
`chain`, `drain_energy`, `remove_hex`, `resurrect`, `lose_adrenaline`.

An amount is `"scale"`, `"bonus"`, `"duration"`, a number (`cast.scale * 2`), `{ low = 5, high = 20 }`
or `{ negate = ... }`. An area (`to`) is one of the catalogue's words (`"target"`, `"self"`,
`"adjacent_foes_of_self"`, `"foes_in_field"`, ...).

A field the reader does not know, or a wrong word, is a script fault: the cast completes doing nothing
and the fault is logged. Only `table`, `string` and `math` are available, and a call has an instruction
budget.
