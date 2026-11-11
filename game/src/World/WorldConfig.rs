//! The world's numbers: every threshold an instance, a district or a spawn uses.
//!
//! Reference: the reference's `WorldConfig` - one typed object a program reads its `World.*` keys into,
//! with the game's value as the default and an accessor per key. This is that object for the numbers the
//! owner's rules name (`docs/game-rules.md`), and it is built the way rule 7 requires: a setting is read
//! from the program's file, never compiled in as a law.
//!
//! **R53 (owner):** every number and threshold stays configurable, with the game's value as the default.
//! So each field below carries the value the game uses, and a program that reads a file that states
//! another value uses that one - and says so in its log.

use common::configuration::Config;

/// The numbers a world runs by. One field per rule that names a number.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldConfig {
    /// How many players one district of an outpost holds before the next district is shown (R5). The game
    /// starts at 100; the allotment is tied to a machine's own efficiency, so it is an operator's number.
    pub district_allotment: u32,
    /// The common party cap (R15). Some game modes allow 12 and some instances 25; a map may state its own
    /// cap in its row, and this is the number that applies when it does not.
    pub party_size_cap: u32,
    /// How far around a spawn point a character is placed on entry (R13), in the game's own units.
    pub spawn_scatter: f32,
    /// Whether an instance keeps a clock at all (R33). The game ticks one; an operator may choose not to.
    pub instance_tick: bool,
    /// The range at which an enemy is simulated (R34), in the game's own units. Outside it, an enemy is not
    /// ticked.
    pub compass_range: f32,
    /// How long an explorable is held for a reconnecting party after a **network** disconnect (R37-R40),
    /// in seconds. A party that leaves by any normal means disposes the instance at once, whatever this
    /// says.
    pub explorable_disconnect_hold: u32,
    /// Whether an outpost that offers a mission presents the "Enter Mission" button (R56).
    pub mission_button: bool,
}

impl Default for WorldConfig {
    /// The game's own numbers, which are what a file that states nothing gets.
    fn default() -> Self {
        Self {
            district_allotment: 100,
            party_size_cap: 8,
            spawn_scatter: 250.0,
            instance_tick: true,
            compass_range: 10_000.0,
            explorable_disconnect_hold: 15 * 60,
            mission_button: true,
        }
    }
}

impl WorldConfig {
    /// Read the `World` section of a program's configuration.
    ///
    /// A key the file does not state keeps the game's value, and the read is reported: the program logs
    /// every number it is running by, so an operator can see the file's effect without reading the code.
    pub fn read(config: &Config) -> Result<(Self, Vec<String>), String> {
        let mut settings = Self::default();
        let mut notes = Vec::new();

        let small = |config: &Config, key: &str, default: u32| -> Result<u32, String> {
            let (value, _) = config
                .value::<u32>(&format!("World.{key}"), default)
                .map_err(|error| error.to_string())?;
            Ok(value)
        };

        settings.district_allotment = small(config, "DistrictAllotment", settings.district_allotment)?;
        settings.party_size_cap = small(config, "PartySizeCap", settings.party_size_cap)?;
        settings.explorable_disconnect_hold =
            small(config, "ExplorableDisconnectHold", settings.explorable_disconnect_hold)?;

        settings.spawn_scatter = number(config, "SpawnScatter", settings.spawn_scatter)?;
        settings.compass_range = number(config, "CompassRange", settings.compass_range)?;
        settings.instance_tick = flag(config, "InstanceTick", settings.instance_tick)?;
        settings.mission_button = flag(config, "MissionButton", settings.mission_button)?;

        notes.push(format!(
            "World: a district holds {} player(s) (R5), a party is capped at {} (R15), a character lands within {:.0} units of its spawn point (R13), an explorable is held {} s for a network disconnect (R37), the compass range is {:.0} (R34), ticking is {}, the mission button is {} (R56).",
            settings.district_allotment,
            settings.party_size_cap,
            settings.spawn_scatter,
            settings.explorable_disconnect_hold,
            settings.compass_range,
            if settings.instance_tick { "on" } else { "off" },
            if settings.mission_button { "shown" } else { "hidden" },
        ));
        Ok((settings, notes))
    }
}

/// One number, as the file states it.
fn number(config: &Config, key: &str, default: f32) -> Result<f32, String> {
    let (text, _) = config
        .value_string(&format!("World.{key}"), &format!("{default}"))
        .map_err(|error| error.to_string())?;
    text.trim()
        .parse()
        .map_err(|_| format!("World.{key} is not a number: '{text}'"))
}

/// One flag, as the file states it: the reader's own `1`/`0`.
fn flag(config: &Config, key: &str, default: bool) -> Result<bool, String> {
    let (value, _) = config
        .value_bool(&format!("World.{key}"), default)
        .map_err(|error| error.to_string())?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::configuration::Policy;

    fn config(text: &str) -> Config {
        Config::parse(text, std::path::PathBuf::from("test.conf"), Policy::default())
    }

    #[test]
    fn the_defaults_are_the_numbers_the_ledger_names() {
        let settings = WorldConfig::default();
        assert_eq!(settings.district_allotment, 100, "R5");
        assert_eq!(settings.party_size_cap, 8, "R15");
        assert_eq!(settings.spawn_scatter, 250.0, "R13");
        assert!(settings.instance_tick, "R33");
        assert_eq!(settings.compass_range, 10_000.0, "R34");
        assert_eq!(settings.explorable_disconnect_hold, 900, "R37");
        assert!(settings.mission_button, "R56");
    }

    #[test]
    fn a_file_that_states_nothing_keeps_the_games_numbers() {
        let (settings, notes) = WorldConfig::read(&config("")).expect("reads");
        assert_eq!(settings, WorldConfig::default());
        assert_eq!(notes.len(), 1, "the read is reported once");
    }

    #[test]
    fn a_file_changes_a_number_and_the_read_says_which() {
        let (settings, notes) = WorldConfig::read(&config(
            "World.DistrictAllotment = 40\nWorld.SpawnScatter = 100\nWorld.InstanceTick = 0\n",
        ))
        .expect("reads");
        assert_eq!(settings.district_allotment, 40);
        assert_eq!(settings.spawn_scatter, 100.0);
        assert!(!settings.instance_tick);
        assert_eq!(settings.party_size_cap, 8, "the keys the file does not state keep the game's value");
        assert!(notes[0].contains("40"), "the log line carries the numbers in force");
    }

    #[test]
    fn a_number_that_is_not_a_number_is_refused_with_its_key() {
        let error = WorldConfig::read(&config("World.SpawnScatter = near\n")).expect_err("refused");
        assert!(error.contains("World.SpawnScatter"), "{error}");
    }
}
