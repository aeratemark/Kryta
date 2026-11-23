-- Hammer Bash (skill 331): attack, profession 1, attribute 19.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:attack{
            bonus = 0,
            on_hit = {
                op.knockdown{ seconds = "duration" },
            },
        }
        cast:lose_adrenaline{}
    end,
}
