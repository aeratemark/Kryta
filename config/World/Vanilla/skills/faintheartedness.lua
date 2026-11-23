-- Faintheartedness (skill 135): hex, profession 4, attribute 7.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:effect{
            to = "target",
            duration = "duration",
            mods = { attack_period = 1.5, health_regen = { negate = "bonus" } },
        }
    end,
}
