-- Ciglo the Monk: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/ciglo_the_monk.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Ciglo the Monk",

    [146] = {
        pages = {
            {
                when = { kind = "quest_reward_open", quest = 84 },
                body = gw(
                    5937, 52450, 51076, 14700, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994,
                    51111, 34011, 18705, 266, 26042, 1, 2, 10994, 51111, 34011, 18705, 266, 25968, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8410119,
                        action = { kind = "quest", quest = 84, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 220 },
                body = gw(
                    5370, 42494, 49609, 15196, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8444935,
                        action = { kind = "quest", quest = 220, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 84 },
                body = gw(
                    5935, 33407, 43098, 17687, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994, 51111,
                    34011, 18705, 266, 26042, 1, 2, 10994, 51111, 34011, 18705, 266, 25968, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5933, 35000, 41424, 21615),
                        id = 8410113,
                        action = { kind = "quest", quest = 84, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5934, 43947, 48581, 13450),
                        id = 8410114,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
