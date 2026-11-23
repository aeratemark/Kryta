-- Fire Storm (skill 197): spell, profession 6, attribute 10.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:field{
            at = "target",
            duration = "duration",
            every_ms = 1000,
            radius = "aoe",
            pulse = {
                op.damage{ to = "foes_in_field", kind = "fire", amount = "scale" },
            },
        }
    end,
}
