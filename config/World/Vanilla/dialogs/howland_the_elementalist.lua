-- Howland the Elementalist: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/howland_the_elementalist.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Howland the Elementalist",

    [146] = {
        pages = {
            {
                when = { kind = "quest_reward_open", quest = 83 },
                body = gw(
                    5925, 41382, 47929, 23104, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994,
                    51111, 34011, 18705, 266, 25466, 1, 2, 10994, 51111, 34011, 18705, 266, 25438, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8409863,
                        action = { kind = "quest", quest = 83, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 219 },
                body = gw(
                    5363, 46788, 45981, 12812, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8444679,
                        action = { kind = "quest", quest = 219, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 83 },
                body = gw(
                    5923, 44441, 56800, 26143, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994, 51111,
                    34011, 18705, 266, 25466, 1, 2, 10994, 51111, 34011, 18705, 266, 25438, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5921, 51049, 44339, 31394),
                        id = 8409857,
                        action = { kind = "quest", quest = 83, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5922, 48233, 35828, 22136),
                        id = 8409858,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
