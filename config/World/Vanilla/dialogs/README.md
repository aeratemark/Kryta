# Dialogs

One Lua file per NPC: what a talk opens and what each choice does, on each map the NPC speaks on.
`scripts/runtime.lua` runs them (`docs/decisions/0019-npc-and-quest-logic-is-lua.md`); the server's
Rust, ported from ASURA, sends what the runtime decides. Read once at start-up: restart the world
after editing a file. A file that does not load or check stops the start, naming the file, the NPC,
the map, the page and the choice.

## Shape

```lua
return {
    npc = "Sir Tydus",              -- the placement's name; a bracketed title is ignored

    [148] = {                       -- the map
        talk_range = 200.0,         -- optional: reach from further than the usual service range
        pages = {
            {                       -- page 1
                when = { kind = "quest_offer_open", quest = 217 },   -- optional
                body = gw(5347, 54311, 59489, 25693),
                buttons = {
                    { icon = 16, label = gw(5351, 62906, 39894, 30816), id = 8444161,
                      action = { kind = "quest", quest = 217, step = "offer" } },
                },
            },
            {                       -- page 2
                body = "Stand fast, recruit.",
                buttons = {
                    { icon = 15, label = "Farewell.", id = 128, action = { kind = "close" } },
                },
            },
        },
    },
}
```

Pages are numbered from 1. A talk opens the first page that is open; a page with choices is only
open while one of them is shown.

## Text: three kinds

| write | is | shows |
|---|---|---|
| `"Welcome to Ascalon."` | a custom string | your own words, sent as literal text. Any character works. When kryta-CS can install strings into `Gw.dat`, these become real string ids and the file does not change |
| `gw(5896, 34250, 47441)` | a real GW string | the game's own text, by the encoded units a capture carried |
| `mix(gw(...), " and ", gw(...))` | a hacked-up GW string | the parts joined in order. **Experimental**: whether the client draws a given combination is only known once it has been tried in-game |

A label holds at most 128 units (a custom string costs its length plus 3).

## `when`: a page's or a choice's condition

| `when` | open while |
|---|---|
| absent | always |
| `{ kind = "quest_offer_open", quest = N }` | the quest can be accepted here now |
| `{ kind = "quest_reward_open", quest = N }` | the quest can be turned in here now |
| `{ kind = "quest_active", quest = N }` | the quest is accepted and not yet complete |
| `{ kind = "answer" }` | never by a talk: only a choice's `page` opens it |
| `function(ctx) return ctx.char.level >= 10 end` | the function says so |

The quests arrive in the next round; until then every quest condition is closed.

`ctx` is `{ map, npc, agent, char = { name, level, primary, secondary, quests } }`.

## `action`: what a choice does

| action | does |
|---|---|
| `{ kind = "close" }` | closes the dialog |
| `{ kind = "page", page = N }` | shows page N |
| `{ kind = "window", window = K }` | opens window kind K owned by the NPC |
| `{ kind = "merchant" }` | opens the NPC's merchant window (`vendors.json`) |
| `{ kind = "quest", quest = N, step = "offer" \| "reward" \| "credit", page = M }` | the quest step, then page M or the next page that offers or rewards a quest |
| `{ kind = "unanswered" }` | a choice no capture shows the answer to: logged, nothing opens |

A choice's `id` is what the client sends back (`0x803B`); `icon` is a byte; `skill` is optional.

## Hooks

```lua
return {
    npc = "Greeter",
    on_talk = function(ctx) if ctx.char.level >= 10 then return 2 end end,   -- a page number, or nil
    on_choose = function(ctx, button) return nil end,                         -- an action, or nil
    [148] = { pages = { ... } },
}
```

`on_talk` may pick the page a talk opens; `on_choose` may answer a choice with an action of its own.
Returning `nil` leaves it to the runtime. Only `table`, `string` and `math` are available, and every
call has an instruction budget.

## Where the 42 committed files came from

`pseudo_lua/presearing dialogs/`, which came from ASURA's `config/World/Vanilla/dialogs.json`
(preforged's pre-Searing captures). Every unit, id, skill and page target was checked against
ASURA's file when they were converted (2026-11-23).
