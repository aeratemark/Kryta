-- runtime.lua - the server's NPC and quest logic, run by the world (game/Scripting/NpcScripts.rs).
--
-- This is the game-logic half of pseudo_lua/runtime.lua, made real (docs/decisions/0019). It decides
-- WHAT happens: which dialog page a talk opens, which choices a character is shown, what a choice
-- does, and (from the next round) the quest stages. It never builds a message: the server's Rust,
-- ported from ASURA, turns every answer here into the wire, in ASURA's order.
--
-- The other half of pseudo_lua/runtime.lua (entry bursts, sight, the tick, behaviour trees, the burrow
-- clock) is NOT here: the server already runs those in Rust (game/Districts/Npcs.rs, game/Behaviour/,
-- npcs/behaviors.lua), and two engines must not drive the same bodies.
--
-- Text. Every body and label is one of three things (game/Scripting/NpcScripts.rs turns them into
-- the client's units):
--
--   "Welcome to Ascalon."     a custom string: your own words, sent as literal text
--   gw(5896, 34250, 47441)    a real GW string: the game's own encoded units, as a capture carried them
--   mix(gw(...), " and ", gw(...))
--                             a hacked-up GW string: pieces joined in order. EXPERIMENTAL - whether the
--                             client draws a given combination is only known once it has been tried.
--
-- The world calls, with ctx = { map, npc, agent, char = { name, level, primary, secondary, quests } }:
--   R.talk(ctx)               -> page number, or nil (no page is open: the NPC has nothing to say)
--   R.view(ctx, n)            -> { body = <text>, buttons = { { icon, label, id, skill }, ... } }
--   R.choose(ctx, n, id)      -> the chosen button's action, or nil (refused: not on the page or hidden)
--   R.next_quest_page(ctx)    -> the first open page that offers or rewards a quest, or nil
--   R.talk_range(map, npc)    -> the dialog's talk_range, or nil
--   R.check()                 -> a list of problems, empty when every dialog checks
--   R.texts()                 -> every text, for the world to check it converts: { where, text, label }
--
-- Plain Lua 5.4, sandboxed: table, string, math. No io, os, require, dofile or load.

local R = {}

R.DIALOGS = {}          -- [map][name] = dialog
R.QUESTS = {}           -- [quest id] = quest; filled by the quests round
R.MAX_BUTTONS = 32      -- gw_npc_dialogue.h: buttons.size() > 32 is invalid (ASURA dialog.rs:19)

-- =============================================================================================
-- Text

function gw(...)
    return { kind = "gw", units = { ... } }
end

function mix(...)
    return { kind = "mix", parts = { ... } }
end

-- =============================================================================================
-- Names. A bracketed title ("Sanura [Merchant]") is not part of the name (ASURA
-- vendor/content.rs base_name). A title missing its closing bracket is cut too.

local function base_name(name)
    name = (name or ""):gsub("^%s+", ""):gsub("%s+$", "")
    local at = name:find(" [", 1, true)
    if at then name = name:sub(1, at - 1):gsub("%s+$", "") end
    return name
end
R.base_name = base_name

-- =============================================================================================
-- Loading

--- One dialog file: { npc = "Name", on_talk = fn?, on_choose = fn?, [map] = { talk_range?, pages } }.
function R.add_dialog(file, t)
    if type(t) ~= "table" then error(file .. ": does not return a table", 0) end
    if type(t.npc) ~= "string" or base_name(t.npc) == "" then
        error(file .. ": has no npc name", 0)
    end
    local name = base_name(t.npc)
    for map, d in pairs(t) do
        if type(map) == "number" then
            if type(d) ~= "table" then error(file .. ": map " .. map .. " is not a table", 0) end
            R.DIALOGS[map] = R.DIALOGS[map] or {}
            if R.DIALOGS[map][name] then
                error(file .. ": " .. name .. " on map " .. map .. " already has a dialog", 0)
            end
            d.npc, d.map, d.file = name, map, file
            d.on_talk = d.on_talk or t.on_talk
            d.on_choose = d.on_choose or t.on_choose
            R.DIALOGS[map][name] = d
        end
    end
end

function R.find(map, npc)
    local by_map = R.DIALOGS[map]
    return by_map and by_map[base_name(npc)]
end

function R.talk_range(map, npc)
    local d = R.find(map, npc)
    return d and d.talk_range
end

-- =============================================================================================
-- Quests (the conditions a page waits for). The quest pack arrives with the quests round; until
-- then R.QUESTS is empty and every quest condition is closed, as ASURA answers with no quests.json.

local function attempt(char, quest)
    return char.quests and char.quests[quest]
end

function R.offer_open(ctx, quest)
    local q = R.QUESTS[quest]
    if not q or not q.offer_open then return false end
    return q.offer_open(ctx) == true
end

function R.reward_open(ctx, quest)
    local q = R.QUESTS[quest]
    if not q or not q.reward_open then return false end
    return q.reward_open(ctx) == true
end

function R.quest_active(ctx, quest)
    local a = attempt(ctx.char, quest)
    return a ~= nil and a.stage == "active"
end

-- =============================================================================================
-- Talking (ASURA dialog.rs When::holds, Page::is_open, Page::visible, Page::has_quest_step)

--- A page's or a choice's condition. `when` is a table (the captured kinds) or a function(ctx).
local function holds(ctx, when, via_choice)
    if when == nil then return true end
    if type(when) == "function" then return when(ctx) == true end
    if when.kind == "answer" then return via_choice == true end
    if when.kind == "quest_offer_open" then return R.offer_open(ctx, when.quest) end
    if when.kind == "quest_reward_open" then return R.reward_open(ctx, when.quest) end
    if when.kind == "quest_active" then return R.quest_active(ctx, when.quest) end
    return false
end

local function visible(ctx, page)
    local out = {}
    for _, b in ipairs(page.buttons or {}) do
        if holds(ctx, b.when) then out[#out + 1] = b end
    end
    return out
end

-- A page is open when its condition holds and, if it has choices, one is still shown: a quest menu
-- whose every quest is closed is not shown (dialog.rs Page::is_open).
local function is_open(ctx, page, via_choice)
    if not holds(ctx, page.when, via_choice) then return false end
    if page.buttons and #page.buttons > 0 then return #visible(ctx, page) > 0 end
    return true
end

local function has_quest_step(ctx, page)
    for _, b in ipairs(visible(ctx, page)) do
        local a = b.action
        if a.kind == "quest" and (a.step == nil or a.step == "offer" or a.step == "reward") then
            return true
        end
    end
    return false
end

--- What a talk opens: the dialog's own on_talk hook first, else the first open page.
function R.talk(ctx)
    local d = R.find(ctx.map, ctx.npc)
    if not d then return nil end
    if d.on_talk then
        local n = d.on_talk(ctx)
        if n ~= nil then
            if type(n) ~= "number" or not d.pages[n] then
                error(d.file .. ": on_talk returned " .. tostring(n) .. ", which is not a page", 0)
            end
            return n
        end
    end
    for n, page in ipairs(d.pages) do
        if is_open(ctx, page, false) then return n end
    end
    return nil
end

--- A page as the character sees it: its body and the choices shown to it.
function R.view(ctx, n)
    local d = R.find(ctx.map, ctx.npc)
    local page = d and d.pages[n]
    if not page then return nil end
    local buttons = {}
    for _, b in ipairs(visible(ctx, page)) do
        buttons[#buttons + 1] = { icon = b.icon, label = b.label, id = b.id, skill = b.skill }
    end
    return { body = page.body, buttons = buttons }
end

--- The choice `id` on page `n`: refused (nil) unless the page shows it to this character. The
--- dialog's on_choose hook may answer instead, with an action of its own.
function R.choose(ctx, n, id)
    local d = R.find(ctx.map, ctx.npc)
    local page = d and d.pages[n]
    if not page then return nil end
    for _, b in ipairs(visible(ctx, page)) do
        if b.id == id then
            if d.on_choose then
                local own = d.on_choose(ctx, b)
                if own ~= nil then return own end
            end
            return b.action
        end
    end
    return nil
end

--- After a quest step: the first open page that offers or rewards a quest (dialog.rs:845-853).
function R.next_quest_page(ctx)
    local d = R.find(ctx.map, ctx.npc)
    if not d then return nil end
    for n, page in ipairs(d.pages) do
        if is_open(ctx, page, false) and has_quest_step(ctx, page) then return n end
    end
    return nil
end

-- =============================================================================================
-- Checking (ASURA dialog.rs DialogFile::check, with this file's 1-based pages)

local ACTIONS = { close = true, window = true, merchant = true, page = true, quest = true, unanswered = true }
local STEPS = { offer = true, reward = true, credit = true }
local WHENS = { quest_offer_open = true, quest_reward_open = true, quest_active = true, answer = true }

local function is_text(v)
    return type(v) == "string" or (type(v) == "table" and (v.kind == "gw" or v.kind == "mix"))
end

function R.check()
    local problems = {}
    local function bad(d, fmt, ...)
        problems[#problems + 1] = string.format("%s (%s, map %d): " .. fmt, d.file, d.npc, d.map, ...)
    end
    local function check_when(d, where, when)
        if when == nil or type(when) == "function" then return end
        if type(when) ~= "table" or not WHENS[when.kind] then
            bad(d, "%s: `when` is not a function or one of quest_offer_open, quest_reward_open, quest_active, answer", where)
        elseif when.kind ~= "answer" and (type(when.quest) ~= "number" or when.quest <= 0) then
            bad(d, "%s: its condition names no quest", where)
        end
    end
    for map, by_name in pairs(R.DIALOGS) do
        for _, d in pairs(by_name) do
            if map <= 0 then bad(d, "map must be positive") end
            if d.talk_range ~= nil and (type(d.talk_range) ~= "number" or d.talk_range <= 0 or d.talk_range ~= d.talk_range) then
                bad(d, "talk_range must be a number above 0")
            end
            if type(d.pages) ~= "table" or #d.pages == 0 then
                bad(d, "no pages")
            else
                for n, page in ipairs(d.pages) do
                    local where = "page " .. n
                    if not is_text(page.body) then bad(d, "%s: body is not a text", where) end
                    check_when(d, where, page.when)
                    local buttons = page.buttons or {}
                    if #buttons > R.MAX_BUTTONS then
                        bad(d, "%s: %d choices, the client takes at most %d", where, #buttons, R.MAX_BUTTONS)
                    end
                    local ids = {}
                    for _, b in ipairs(buttons) do
                        local at = string.format("%s choice %s", where, tostring(b.id))
                        if type(b.id) ~= "number" or b.id <= 0 or b.id > 0xFFFFFFFF then
                            bad(d, "%s: the id must be a number from 1 to 0xFFFFFFFF", at)
                        elseif ids[b.id] then
                            bad(d, "%s: the id is named twice", at)
                        end
                        ids[b.id] = true
                        if type(b.icon) ~= "number" or b.icon < 0 or b.icon > 255 then
                            bad(d, "%s: icon must be a number from 0 to 255", at)
                        end
                        if not is_text(b.label) then bad(d, "%s: label is not a text", at) end
                        if b.when ~= nil and type(b.when) == "table" and b.when.kind == "answer" then
                            bad(d, "%s: waits for answer, which only a page can", at)
                        end
                        check_when(d, at, b.when)
                        local a = b.action
                        if type(a) ~= "table" or not ACTIONS[a.kind] then
                            bad(d, "%s: action is not one of close, window, merchant, page, quest, unanswered", at)
                        elseif a.kind == "window" and (type(a.window) ~= "number" or a.window <= 0 or a.window > 255) then
                            bad(d, "%s: opens window kind %s", at, tostring(a.window))
                        elseif a.kind == "page" and (type(a.page) ~= "number" or not d.pages[a.page]) then
                            bad(d, "%s: names page %s, and there are %d", at, tostring(a.page), #d.pages)
                        elseif a.kind == "quest" then
                            if type(a.quest) ~= "number" or a.quest <= 0 then bad(d, "%s: names no quest", at) end
                            if a.step ~= nil and not STEPS[a.step] then bad(d, "%s: step is not offer, reward or credit", at) end
                            if a.page ~= nil and (type(a.page) ~= "number" or not d.pages[a.page]) then
                                bad(d, "%s: names page %s, and there are %d", at, tostring(a.page), #d.pages)
                            end
                        end
                    end
                end
            end
        end
    end
    table.sort(problems)
    return problems
end

--- Every text, so the world can check each converts to units the client takes.
function R.texts()
    local out = {}
    for _, by_name in pairs(R.DIALOGS) do
        for _, d in pairs(by_name) do
            for n, page in ipairs(d.pages or {}) do
                local where = string.format("%s (%s, map %d) page %d", d.file, d.npc, d.map, n)
                out[#out + 1] = { where = where, text = page.body, label = false }
                for _, b in ipairs(page.buttons or {}) do
                    out[#out + 1] = { where = where .. " choice " .. tostring(b.id), text = b.label, label = true }
                end
            end
        end
    end
    return out
end

--- How many dialogs are loaded, for the start-up line.
function R.count()
    local dialogs, npcs = 0, {}
    for _, by_name in pairs(R.DIALOGS) do
        for name in pairs(by_name) do
            dialogs = dialogs + 1
            npcs[name] = true
        end
    end
    local n = 0
    for _ in pairs(npcs) do n = n + 1 end
    return dialogs, n
end

return R
