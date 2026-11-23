-- Imagined Burden (skill 76): hex, profession 5, attribute 1.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:effect{
            to = "target_and_adjacent_foes",
            duration = "duration",
            mods = { move_speed = 0.5 },
        }
    end,
}
