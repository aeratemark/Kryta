-- Kasha Blackblood: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/kasha_blackblood.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Kasha Blackblood",

    [160] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 13019, 49828, 61563, 24611, 1),
                buttons = {
                    {
                        icon = 18,
                        label = gw(5511, 52827, 34454, 31531),
                        id = 8399619,
                        action = { kind = "page", page = 3 },
                        when = { kind = "quest_offer_open", quest = 43 },
                    },
                    {
                        icon = 11,
                        label = gw(13028, 34263, 56016, 12530),
                        id = 129,
                        action = { kind = "close" },
                    },
                    {
                        icon = 21,
                        label = gw(6007, 34027, 54662, 5611),
                        id = 8408836,
                        action = { kind = "quest", quest = 79, step = "credit", page = 4 },
                        when = { kind = "quest_active", quest = 79 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 43 },
                body = gw(
                    5514, 51896, 47670, 24450, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8399623,
                        action = { kind = "quest", quest = 43, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 43 },
                body = gw(
                    5512, 49788, 43452, 7268, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5517, 37047, 41771, 18701),
                        id = 8399617,
                        action = { kind = "quest", quest = 43, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5518, 43355, 34235, 27796),
                        id = 8399618,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(6019, 40901, 38206, 31896), buttons = {} },
        },
    },
}
