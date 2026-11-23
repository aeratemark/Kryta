-- Shatter Delusions (skill 27): spell, profession 5, attribute 2.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:remove_hex{
            to = "target",
            profession = 5,
            ["then"] = {
                op.damage{
                    to = "target_and_adjacent_foes",
                    kind = "untyped",
                    amount = "scale",
                },
            },
        }
    end,
}
