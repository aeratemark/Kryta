-- Sir Tydus: what a talk opens and what each choice does, per map.
--
-- Real Lua, loaded by the world (game/Scripting/NpcScripts.rs) and run by scripts/runtime.lua.
-- Converted 2026-11-23 from pseudo_lua/presearing dialogs/sir_tydus.lua, which came from ASURA
-- config/World/Vanilla/dialogs.json (tools/npc-content-from-captures.py over preforged's pre-Searing
-- captures). The README in this folder is the whole format.
--
-- gw(...) is the game's own text, as the capture carried it. A page's `when` waits for a quest state;
-- the quests arrive in the next round, so until then a quest page stays closed. A choice's `id` is
-- what the client sends back in 0x803B. Pages are numbered from 1.

return {
    npc = "Sir Tydus",

    [148] = {
        talk_range = 200.0,
        pages = {
            {
                when = { kind = "quest_reward_open", quest = 80 },
                body = gw(
                    5896, 34250, 47441, 14233, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 356, 2, 10988,
                    56007, 33198, 13442, 257, 266
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8409095,
                        action = { kind = "quest", quest = 80, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_reward_open", quest = 89 },
                body = gw(
                    6045, 45517, 60453, 13602, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 258, 2,
                    258, 2, 10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 356
                ),
                buttons = {
                    {
                        icon = 23,
                        label = gw(10975, 59449, 46843, 6569),
                        id = 8411399,
                        action = { kind = "quest", quest = 89, step = "reward" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 217 },
                body = gw(
                    5347, 54311, 59489, 25693, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5351, 62906, 39894, 30816),
                        id = 8444161,
                        action = { kind = "quest", quest = 217, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5352, 61092, 35723, 23297),
                        id = 8444162,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 218 },
                body = gw(
                    5354, 45083, 51068, 25254, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5358, 41458, 47318, 26784),
                        id = 8444417,
                        action = { kind = "quest", quest = 218, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5359, 63583, 55894, 4492),
                        id = 8444418,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 219 },
                body = gw(
                    5361, 40130, 39721, 13721, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5365, 51166, 54899, 5249),
                        id = 8444673,
                        action = { kind = "quest", quest = 219, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5366, 55110, 44041, 423),
                        id = 8444674,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 220 },
                body = gw(
                    5368, 52509, 37430, 20037, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5372, 48309, 48993, 25088),
                        id = 8444929,
                        action = { kind = "quest", quest = 220, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5373, 47532, 42790, 10160),
                        id = 8444930,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 221 },
                body = gw(
                    5375, 63483, 57068, 22817, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5379, 39474, 38769, 19988),
                        id = 8445185,
                        action = { kind = "quest", quest = 221, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5380, 65408, 65159, 12802),
                        id = 8445186,
                        action = { kind = "close" },
                    },
                },
            },
            {
                when = { kind = "quest_offer_open", quest = 222 },
                body = gw(
                    5382, 53548, 36182, 19581, 266, 2985, 263, 76, 111, 103, 103, 101, 114, 1, 1, 2, 263, 2,
                    10984, 59348, 58828, 13938, 2, 10986, 35903, 46361, 26129, 257, 506, 2, 263
                ),
                buttons = {
                    {
                        icon = 16,
                        label = gw(5386, 49083, 51955, 3136),
                        id = 8445441,
                        action = { kind = "quest", quest = 222, step = "offer" },
                    },
                    {
                        icon = 17,
                        label = gw(5387, 63528, 34450, 32065),
                        id = 8445442,
                        action = { kind = "close" },
                    },
                },
            },
            {
                body = gw(13188, 46997, 46533, 25935),
                buttons = {
                    {
                        icon = 24,
                        label = gw(13190, 43607, 34750, 28223),
                        id = 132,
                        action = { kind = "page", page = 10 },
                    },
                    {
                        icon = 25,
                        label = gw(13192, 34554, 55965, 16879),
                        id = 128,
                        action = { kind = "close" },
                    },
                },
            },
            { when = { kind = "answer" }, body = gw(13204, 64798, 49181, 29737), buttons = {} },
        },
    },
}
