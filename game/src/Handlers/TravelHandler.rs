//! The travel family: a transfer from one map to another, and the three things that can start one.
//!
//! Reference: `src/server/game/Handlers/` has no travel file of its own - a transfer is a game object's
//! operation there (`Player::TeleportTo`, and the map-to-map hand-off) driven by the handler that saw
//! the request. The behaviours here are ASURA's, ported whole, and its own files say why they are one
//! cycle and not one per trigger: `ASURA/crates/asura-world/src/travel.rs` - `/tp`
//! (`answer_teleport_command`, `travel.rs:184-258`), the world map's request (`answer_party_travel`,
//! `:279-302`), the travel object's touch (`answer_travel_init`, `:132-169`), and the doorway a step
//! crossed (`cross_portal`, `:319-388`). Every one of them ends in `begin_transfer` (`:401-534`).
//!
//! **No frame, order or size changed when this moved out of `WorldSession.rs`.**

use common::logging::LogLevel;
use shared::cryptography::AuthCrypt;
use shared::network::socket::SessionIo;

use crate::server::world_session::CharacterSheet;
use crate::server::packets::{
    agent_packets, character_packets, misc_packets, party_packets,
};
use crate::server::world_session::{parse_address, WorldSession, TRAVEL_EFFECT};

impl WorldSession {
    /// A crossing decided from the player's **own step**: where the character stood before this report
    /// and where it stands after it.
    ///
    /// `ASURA/crates/asura-world/src/travel.rs:319-388` (`portal_crossed`), ported whole: a crossing
    /// already in flight owns the connection, the map's own arches decide whether the step crosses a
    /// wall, an arch that names no destination does nothing and says so, and the receiving map's
    /// arrival decides where the traveller stands. It ends in the same `begin_transfer` every other
    /// trigger ends in, because the reference has one cycle and not one per trigger.
    pub(crate) async fn cross_portal(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
        from: (f32, f32),
        to: (f32, f32),
    ) {
        if let Ok(moved) = self.moved.lock()
            && *moved
        {
            return;
        }
        let from_map = self.map_id();
        let Some(crossed) = self.portals.crossed(from_map, from, to) else {
            return;
        };
        let Some(to_map) = crossed.to_map else {
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: the character crossed a doorway of map {from_map} from ({:.0}, {:.0}) to ({:.0}, {:.0}); no destination is recorded for that doorway.",
                    from.0, from.1, to.0, to.1
                ),
            );
            return;
        };
        // The receiving map decides where the traveller stands: an arrival it states for this map is a
        // position and a facing; without one the destination falls back to its own spawn point.
        let arrival = self
            .arrivals
            .iter()
            .find(|arrival| arrival.map_id == to_map && arrival.from_map == from_map)
            .copied();
        let landing = arrival.map(|arrival| (arrival.x, arrival.y));
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: the character crossed the doorway of map {from_map} at ({:.0}, {:.0}) -> ({:.0}, {:.0}): crossing to {to_map}{}",
                from.0,
                from.1,
                to.0,
                to.1,
                match arrival {
                    Some(arrival) => format!(
                        " (arrival at ({:.1}, {:.1}) facing {:.2})",
                        arrival.x,
                        arrival.y,
                        arrival.angle.unwrap_or(0.0)
                    ),
                    None => " (no arrival stated; the destination's spawn point answers)".to_string(),
                }
            ),
        );
        let Some(destination) = self
            .maps
            .iter()
            .find(|map| map.id == to_map && map.is_hostable())
            .cloned()
        else {
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: instance {to_map} cannot be served by this world; the crossing is refused."
                ),
            );
            return;
        };
        self.begin_transfer(io, peer, crypt, sheet, &destination, landing)
            .await;
    }

    /// `GAME_CMSG_PARTY_TRAVEL` (`0x80B1`, nine bytes): the client asks for an instance from the
    /// world map.
    ///
    /// `ASURA/crates/asura-world/src/travel.rs`, `handle_party_travel`: the first two bytes are the
    /// instance it wants; a map this world does not host is refused by saying so, and a map it hosts goes
    /// to `begin_transfer`. The nine bytes are the client's own frame - `B1 80 | 94 00 | 02 | 00 00 | 0...`
    /// in the working implementation's log - of which only the instance is read here, exactly as there.
    pub(crate) async fn answer_party_travel(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let body = &message[2..];
        if body.len() < 2 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a travel request is {} bytes and names no instance; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let map_id = u32::from(u16::from_le_bytes([body[0], body[1]]));
        // A record the client can resolve and this world cannot load is not servable: ASURA's own
        // `catalogue::entry` answers `None` for a map with no archive file
        // (`ASURA/crates/asura-world/src/catalogue.rs:54-63`).
        let Some(destination) = self
            .maps
            .iter()
            .find(|map| map.id == map_id && map.is_hostable())
            .cloned()
        else {
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: instance {map_id} cannot be served by this world; the travel is refused."
                ),
            );
            return;
        };
        self.begin_transfer(io, peer, crypt, sheet, &destination, None).await;
    }

    /// The frames a travel puts on the wire, in the working implementation's own order
    /// (`ASURA/crates/asura-world/src/travel.rs:480-518`): the player's agent takes the travel effect,
    /// its walk stops, the redirect names where to reconnect and with what token, the map update follows
    /// in its twelve-byte form, and the travelling body is withdrawn from the map it is leaving - the party
    /// size, the party, the player and the agent.
    async fn begin_transfer(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
        destination: &crate::world::MapEntry,
        landing: Option<(f32, f32)>,
    ) {
        // A travel already in flight owns this connection: the client is about to reconnect, and a
        // second redirect would send it two.
        match self.moved.lock() {
            Ok(mut moved) if !*moved => *moved = true,
            _ => return,
        }

        // The address the client will dial. The working implementation publishes one
        // (`--public`, `ASURA/crates/asura-world/src/main.rs:73`); this world states it in its own file,
        // and this stack's client dials the address it was launched against (`docs/port/02-wire.md`).
        let Some((host, port)) = parse_address(&self.settings.public_address) else {
            self.say(
                LogLevel::Error,
                format!(
                    "Gameplay connection from {peer}: World.PublicAddress is '{}', which is not an IPv4 address and port; the travel is refused.",
                    self.settings.public_address
                ),
            );
            return;
        };

        // A number drawn per travel, so the world can tell one client's connection from another's: the
        // arriving connection presents it, and the hand-off row is what names the map it is for.
        let mut token_bytes = [0u8; 4];
        if getrandom::getrandom(&mut token_bytes).is_err() {
            self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: no entropy for the player token; using zero."),
            );
        }
        let player_token = u32::from_le_bytes(token_bytes);
        let account = match self.account.lock() {
            Ok(account) => *account,
            Err(_) => [0u8; 16],
        };
        let language = 0;
        let district = 0;

        // The hand-off, written before the redirect: the connection the client is about to open reads it
        // to learn the map and the district. It is the login database's row, because that is where this
        // stack's login writes the one a client asks for (`docs/port/06-world-plan.md`, "The three seams").
        let mut recorded = false;
        if let Some(login) = self.login.as_ref() {
            let travels = crate::maps::DatabaseTravels::new(
                std::sync::Arc::clone(login),
                self.log.clone(),
            );
            let travel = crate::maps::Travel {
                account,
                world: self.realm,
                map_id: destination.id,
                district,
                language,
                player_token,
                // The landing a crossing hands over; every other trigger states none, which clears
                // the saved position rather than keeping it (travel.rs:396-400, 446-468).
                landing: landing.map(|(x, y)| crate::maps::travels::Landing {
                    x,
                    y,
                    plane: 0,
                    from_map: self.map_id(),
                }),
            };
            match travels.record(&travel) {
                Ok(()) => {
                    recorded = true;
                    self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: the travel to map {} is recorded for world {}; the world claims it when the connection presents token 0x{player_token:08X}.",
                            destination.id, self.realm
                        ),
                    );
                }
                Err(error) => self.say(
                    LogLevel::Error,
                    format!(
                        "Gameplay connection from {peer}: the travel to map {} was not recorded ({error}); the client would reconnect with no hand-off, so the redirect is not sent.",
                        destination.id
                    ),
                ),
            }
        }
        if !recorded {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a travel to map {} cannot be handed off without the login database; the connection stays where it is.",
                    destination.id
                ),
            );
            return;
        }

        // The arriving position is the destination's to decide, and it is written before the redirect so
        // the connection the client is about to open reads it (`ASURA/crates/asura-world/src/travel.rs:446-468`).
        // A crossing hands over its landing; every other travel states none, which **clears** the saved
        // position - a travel does not preserve where a character was, only a disconnect does.
        self.place_for_travel(destination, landing, sheet);

        let agent = self.player_id();
        let player = self.player_id();
        let is_explorable = self.settings.travel_explorable.flag(destination.kind.unwrap_or(0));
        let frames = vec![
            agent_packets::agent_update_effects(agent, TRAVEL_EFFECT).1,
            agent_packets::agent_stop_moving(agent).1,
            misc_packets::transfer_game_server_info(
                host,
                port,
                self.realm,
                0, // region: the reference sends its district's region, and this world has no regions
                player_token,
                is_explorable,
                destination.id,
            )
            .1,
            misc_packets::map_update_current_redirect(destination.id).1,
            party_packets::player_update_party_size(player, 1),
            party_packets::player_update_party(player, 0),
            character_packets::player_destroy(player).1,
            agent_packets::world_remove_agent(agent).1,
        ];
        self.send(io, peer, crypt, &frames, "the redirect").await;
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: the character '{}' travels to map {} ('{}'); the client is told to dial {}.{}.{}.{}:{} with token 0x{player_token:08X}.",
                sheet.name,
                destination.id,
                destination.name,
                host[0],
                host[1],
                host[2],
                host[3],
                port
            ),
        );
    }

    /// `GAME_CMSG_TRAVEL_INIT` (`0x80C1`, ten bytes): the client touched a travel object - a portal, a
    /// door, a signpost.
    ///
    /// `ASURA/crates/asura-world/src/travel.rs:132-169` (`handle_travel_init`), ported whole: the two
    /// agent ids are read - the object the player used, and a second one that has been zero in every
    /// measurement - and the touch is named in the log. **It names an agent, not a map**, so the
    /// destination is this server's knowledge and not the client's: the object it names leads somewhere
    /// only because a row says so. The doorway crossing itself is decided from the player's own step
    /// (`travel.rs:319-388`, `portal_crossed`), not from this message, so this arm starts no transfer -
    /// which is exactly what it does in the working implementation.
    pub(crate) async fn answer_travel_init(&self, peer: &str, message: &[u8], sheet: &CharacterSheet) {
        let body = &message[2..];
        if body.len() < 8 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a travel touch is {} bytes and needs 10; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let agent = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let other = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: the character '{}' touched a travel object in map {}: TRAVEL_INIT agent 0x{agent:08X}, second 0x{other:08X}; no portal row names that agent, so no travel starts.",
                sheet.name,
                self.map_id()
            ),
        );
    }

    /// `/tp <alias>` and `/teleport <alias>`: the reference's `GameSrv_HandleTeleportCommand`
    /// (`GmCommands.c:155-216`), ported whole from
    /// `ASURA/crates/asura-world/src/travel.rs:184-258` (`teleport`).
    ///
    /// A teleport is not a position change - it is a **server transfer**, the same cycle a portal
    /// starts, so it ends in `begin_transfer` exactly as the client's own request does. The reference's
    /// three refusals are here verbatim, in its order: an argument that names nothing, an alias its
    /// table does not list (`Unknown map name. Type /maps.`), a target with no map config, a target
    /// this world has no row for, and finally the command's own "You are already here."
    ///
    /// What is not here, and is named rather than hidden: the working implementation also loads a
    /// destination on demand (`--on-demand`, `travel.rs:216-247`); this world has no such switch, so a
    /// map it has no row for is refused instead of loaded.
    pub(crate) async fn answer_teleport_command(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
        argument: &str,
    ) {
        if crate::chat::alias_key(argument).is_empty() {
            // `GmCommand_ToAscii` refuses an empty argument, and the answer is the invalid-command line.
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: /tp without a map name."),
            );
            return;
        }
        // The alias table, not the map names: the reference accepts `lakeside` as well as
        // `lakeside county`, so matching against a map's own name would refuse half the aliases its
        // own `/maps` prints.
        let Some(target) = crate::chat::travel_target(argument) else {
            // `GmCommands.c:172-177`: "Unknown map name. Type /maps." through the gated chat sender,
            // which with the gate off is a log line.
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: Unknown map name. Type /maps."),
            );
            return;
        };
        if !target.hosted {
            // `GmCommands.c:180-190`: the reference refuses a target with no `GmMapConfig` rather than
            // letting its map load fall back to Kamadan.
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: That map has no config; the server would fall back, so the teleport is refused ({}).",
                    target.label
                ),
            );
            return;
        }
        let Some(destination) = self
            .maps
            .iter()
            .find(|map| map.id == target.map_id && map.is_hostable())
            .cloned()
        else {
            // The table knows the map; this world hosts no row for it. The reference pre-spawns one
            // server per map, so its answer would be a transfer; this one can only say so.
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: {} ({}) is not hosted by this world; the teleport is refused.",
                    target.label, target.map_id
                ),
            );
            return;
        };
        if target.map_id == self.map_id() {
            // `GmCommands.c:198-202`. **The command's own refusal, not a travel policy**: the
            // client-driven requests (`0x80B1`) are carried out without this check, because there the
            // client has already decided and a second opinion is a gate. A typed alias that names the
            // instance the character stands in is the reference's "You are already here." and nothing
            // more.
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: You are already here."),
            );
            return;
        }
        self.begin_transfer(io, peer, crypt, sheet, &destination, None).await;
    }
}
