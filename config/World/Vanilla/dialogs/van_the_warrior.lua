-- Van the Warrior: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/van_the_warrior.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Van the Warrior",

    [146] = {
        pages = {
            {
                when = { kind = "quest_reward_open", quest = 85 },
                body = gw(
                    5949, 46152, 64684, 17249, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994,
                    51111, 34011, 18705, 266, 26161, 1, 2, 10994, 51111, 34011, 18705, 266, 25218, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8410375,
                        action = { kind = "quest", quest = 85, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 221 },
                body = gw(
                    5377, 56787, 37972, 20128, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8445191,
                        action = { kind = "quest", quest = 221, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 85 },
                body = gw(
                    5947, 59619, 52150, 23184, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994, 51111,
                    34011, 18705, 266, 26161, 1, 2, 10994, 51111, 34011, 18705, 266, 25218, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5945, 47735, 48144, 17838),
                        id = 8410369,
                        action = { kind = "quest", quest = 85, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5946, 40801, 49211, 12281),
                        id = 8410370,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
