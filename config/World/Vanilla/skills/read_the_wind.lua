-- Read the Wind (skill 432): preparation, profession 2, attribute 25.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:effect{
            to = "self",
            duration = "duration",
            mods = { arrow_damage = "bonus" },
        }
    end,
}
