-- Dual Shot (skill 396): attack, profession 2, attribute 51.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:attack{ bonus = 0, strikes = 2, damage_multiplier = 0.75 }
    end,
}
