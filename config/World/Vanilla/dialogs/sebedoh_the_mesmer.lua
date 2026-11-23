-- Sebedoh the Mesmer: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/sebedoh_the_mesmer.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Sebedoh the Mesmer",

    [146] = {
        pages = {
            {
                when = { kind = "quest_reward_open", quest = 81 },
                body = gw(
                    5903, 61578, 40445, 843, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2, 258,
                    2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994, 51111,
                    34011, 18705, 266, 25103, 1, 2, 10994, 51111, 34011, 18705, 266, 25131, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8409351,
                        action = { kind = "quest", quest = 81, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 217 },
                body = gw(
                    5349, 54046, 34738, 16751, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8444167,
                        action = { kind = "quest", quest = 217, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 81 },
                body = gw(
                    5901, 36695, 33321, 3997, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994, 51111,
                    34011, 18705, 266, 25103, 1, 2, 10994, 51111, 34011, 18705, 266, 25131, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5899, 43156, 55969, 9208),
                        id = 8409345,
                        action = { kind = "quest", quest = 81, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5900, 37328, 52489, 30851),
                        id = 8409346,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
