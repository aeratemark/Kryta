-- Artemis the Ranger: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/artemis_the_ranger.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Artemis the Ranger",

    [146] = {
        pages = {
            {
                body = gw(10982, 63947, 59705, 24018, 266, 5963, 52425, 56859, 17134, 1),
                buttons = {
                    {
                        icon = 22,
                        label = gw(5955, 36406, 55959, 7191),
                        id = 8410629,
                        action = { kind = "close" },
                        when = { kind = "quest_active", quest = 86 },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 86 },
                body = gw(
                    5960, 48563, 34856, 10076, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 756, 2, 10994,
                    51111, 34011, 18705, 266, 26402, 1, 2, 10994, 51111, 34011, 18705, 266, 26506, 1
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8410631,
                        action = { kind = "quest", quest = 86, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 222 },
                body = gw(
                    5384, 50792, 53298, 10561, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8445447,
                        action = { kind = "quest", quest = 222, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 86 },
                body = gw(
                    5958, 39758, 50883, 21044, 266, 2985, 263, 81, 117, 101, 115, 116, 32, 76, 111, 103, 103,
                    101, 114, 1, 1, 2, 263, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257,
                    756, 2, 10994, 51111, 34011, 18705, 266, 26402, 1, 2, 10994, 51111, 34011, 18705, 266,
                    26506, 1, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5956, 37944, 63339, 6616),
                        id = 8410625,
                        action = { kind = "quest", quest = 86, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5957, 41286, 59076, 26373),
                        id = 8410626,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(10982, 63947, 59705, 24018, 266, 5963, 52425, 56859, 17134, 1),
                buttons = {
                    {
                        icon = 15,
                        label = gw(10981, 56770, 59190, 3361),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
        },
    },
}
