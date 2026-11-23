-- Troll Unguent (skill 446): skill, profession 2, attribute 24.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:effect{
            to = "self",
            duration = "duration",
            mods = { health_regen = "scale" },
        }
    end,
}
