-- Gash (skill 384): attack, profession 1, attribute 20.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:attack{
            bonus = "scale",
            on_hit = {
                op.condition{
                    condition = "deep_wound",
                    duration = "bonus",
                    if_target = "bleeding",
                },
            },
        }
    end,
}
