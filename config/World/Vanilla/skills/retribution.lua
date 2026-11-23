-- Retribution (skill 248): enchantment, profession 3, attribute 14.
--
-- Translated one-to-one from ASURA_testfork's `config/World/Vanilla/skills.json` `effects` list for this
-- skill: each `cast:<op>{...}` queues the operation of the same name and fields, which the engine runs
-- exactly as ASURA runs its declared one. The API is `config/World/Vanilla/skills/README.md`.

return {
    on_use = function(cast)
        cast:effect{
            to = "target",
            maintained = true,
            mods = { attacker_damage = { low = 5, high = 20 } },
        }
    end,
}
