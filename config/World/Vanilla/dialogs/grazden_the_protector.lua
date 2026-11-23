-- Grazden the Protector: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/grazden_the_protector.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Grazden the Protector",

    [160] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13030, 59528, 56676, 27642, 1),
                buttons = {
                    {
                        icon = 21,
                        label = gw(6007, 34027, 54662, 5611),
                        id = 8408836,
                        action = { kind = "quest", quest = 79, step = "credit", page = 4 },
                        when = { kind = "quest_active", quest = 79 },
                    },
                    {
                        icon = 11,
                        label = gw(13039, 37998, 35957, 28039),
                        id = 129,
                        action = { kind = "close" },
                    },
                    {
                        icon = 18,
                        label = gw(5867, 47637, 57260, 32073),
                        id = 8408323,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_offer_open", quest = 77 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 77 },
                body = gw(
                    5872, 37266, 51126, 7431, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8408327,
                        action = { kind = "quest", quest = 77, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 77 },
                body = gw(
                    5870, 45057, 56840, 5827, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5868, 55281, 50876, 5084),
                        id = 8408321,
                        action = { kind = "quest", quest = 77, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5869, 40903, 38335, 22395),
                        id = 8408322,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(6021, 43771, 60100, 3329), buttons = {} },
        },
    },
}
