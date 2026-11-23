-- Duke Gaban: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/duke_gaban.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Duke Gaban",

    [146] = {
        pages = {
            {
                when = { kind = "quest_reward_open", quest = 51 },
                body = gw(
                    5616, 60998, 56613, 5244, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 406, 2, 10990, 57672,
                    47647, 9095, 257, 258, 266, 9548, 61784, 56161, 30436, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8401671,
                        action = { kind = "quest", quest = 51, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 51 },
                body = gw(
                    5611, 63831, 63648, 21395, 266, 2302, 1, 2, 263, 2, 10984, 59348, 58828, 13938, 2, 10986,
                    35903, 46361, 26129, 257, 406, 2, 10990, 57672, 47647, 9095, 257, 258, 266, 9548, 61784,
                    56161, 30436, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5612, 47345, 47435, 7621),
                        id = 8401665,
                        action = { kind = "quest", quest = 51, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5613, 39823, 40178, 13810),
                        id = 8401666,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
