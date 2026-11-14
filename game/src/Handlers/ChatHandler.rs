//! The chat family: `GAME_CMSG_SEND_CHAT_MESSAGE` (`0x8064`), a spoken line or one of the reference's
//! own commands.
//!
//! Reference: `src/server/game/Handlers/ChatHandler.cpp`, which reads the message, applies the language
//! and command gates, and calls into the chat object. The behaviour is ASURA's, ported whole:
//! `ASURA/crates/asura-world/src/chat.rs:193-353`, whose channel rule, command table and answers are
//! `game/src/Chat/`. A command that teleports ends in `answer_teleport_command`, which is the travel
//! family's (`Handlers/TravelHandler.rs`), exactly as ASURA keeps it in `travel.rs`.
//!
//! **No frame, order or size changed when this moved out of `WorldSession.rs`.**

use common::logging::LogLevel;
use shared::cryptography::AuthCrypt;
use shared::network::socket::SessionIo;

use crate::server::packets::{agent_packets, chat_packets};
use crate::server::world_session::{CharacterSheet, WorldSession};

impl WorldSession {    /// `GAME_CMSG_SEND_CHAT_MESSAGE` (`0x8064`): the client speaks.
    ///
    /// Ported whole from `ASURA/crates/asura-world/src/chat.rs:193-353` (`handle_chat_message`). The
    /// frame is `{agent u32, text String16}` - the table's own field list
    /// (`ASURA/docs/reference/client-protocol.md:920-926`), which is why the text's count sits four
    /// bytes in - and the **first unit of the text is the channel**: `!` all, `#` group, `$` trade,
    /// `/` a command, `@`/`%`/`"` dropped as the reference drops them, and anything else refused.
    ///
    /// A player's line is **sent**, exactly as the reference sends it: the text (`0x005D`) and who spoke
    /// it (`0x0061`), because the reference's own chat gate covers the text the *server* generates and
    /// not a line a player typed. Every answer the server generates is printed instead - `/maps`' table
    /// and every refusal - because that is what the reference's gate does with them.
    pub(crate) async fn answer_chat_message(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let body = &message[2..];
        if body.len() < 6 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a chat line is {} bytes and carries no text; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let units = u16::from_le_bytes([body[4], body[5]]) as usize;
        let text: Vec<u16> = (0..units)
            .map(|index| {
                let at = 6 + index * 2;
                u16::from_le_bytes([
                    body.get(at).copied().unwrap_or(0),
                    body.get(at + 1).copied().unwrap_or(0),
                ])
            })
            .collect();
        let Some(channel) = text.first().copied() else {
            // An empty message, which the reference drops.
            return;
        };
        let spoken: String = char::decode_utf16(text[1.min(text.len())..].iter().copied())
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();

        match u8::try_from(channel).unwrap_or(0) as char {
            // `Channel_All`, `Channel_Group`, `Channel_Trade`: the three channels the reference speaks
            // on. Sent ungated, as the reference sends them.
            speaker @ ('!' | '#' | '$') => {
                let channel = match speaker {
                    '#' => chat_packets::CHANNEL_GROUP,
                    '$' => chat_packets::CHANNEL_TRADE,
                    _ => chat_packets::CHANNEL_ALL,
                };
                // The player's id and its agent's are one number here (`self.player_id()`, the id the
                // load drew the body as); the working implementation reads them from its player.
                let frames = vec![
                    chat_packets::chat_message_core(&spoken),
                    chat_packets::chat_message_local(self.player_id(), channel),
                ];
                self.send(io, peer, crypt, &frames, "a spoken line").await;
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: {} spoke {spoken:?} on channel {channel}.",
                        sheet.name
                    ),
                );
            }
            '/' => {
                // The command surface, from the reference's own two places: `GmCommands.c`'s
                // `GameSrv_TryHandleGmCommand` (`/tp`, `/teleport`, `/maps`) and `GmChat.c`'s
                // `GameSrv_HandleChatCommand` (`/stuck`). `crate::chat::COMMANDS` names them once.
                let spoken = spoken.trim();
                let (name, argument) = match spoken.split_once(' ') {
                    Some((name, rest)) => (name, rest.trim_start()),
                    None => (spoken, ""),
                };
                match crate::chat::command(name) {
                    Some(named) if named.names.contains(&"stuck") && argument.is_empty() => {
                        // `GameSrv_HandleChatCommand`'s `stuck`: broadcast where the agent is now.
                        // **Not** `AGENT_STOP_MOVING` - that is what `0x8028` answers - and the working
                        // implementation sends the position alone (`chat.rs:260-287`); the tick it also
                        // broadcasts is the ticker's own, so no second clock is invented.
                        let Some(frame) = self.with_agent(|agent| {
                            agent_packets::agent_update_position(
                                agent.agent_id,
                                agent.position,
                                agent.plane,
                            )
                            .1
                        }) else {
                            return;
                        };
                        self.send(io, peer, crypt, &[frame], "where the agent stands")
                            .await;
                    }
                    Some(named) if named.names.contains(&"maps") && argument.is_empty() => {
                        // `GmCommand_ListTargets`, as it behaves with the reference's own defaults: the
                        // table goes to the log, and nothing goes on the wire. That is parity, not a
                        // gate - sending it crashed a live client, which is why the answer is printed
                        // (`chat.rs:288-324`). The chat half is written too, and printed at debug,
                        // which is the level this tree's traces are read at.
                        let answer = crate::chat::maps_answer(&self.maps);
                        for line in &answer.table {
                            self.say(LogLevel::Info, format!("Gameplay connection from {peer}: {line}"));
                        }
                        for line in &answer.chat {
                            self.say(
                                LogLevel::Debug,
                                format!(
                                    "Gameplay connection from {peer}: /maps (chat, not sent): {line}"
                                ),
                            );
                        }
                    }
                    Some(named) if named.names.contains(&"tp") && !argument.is_empty() => {
                        self.answer_teleport_command(io, peer, crypt, sheet, argument)
                            .await;
                    }
                    _ => {
                        // `GameSrv_SendInvalidCommand`: the reference's text is "Unknown command",
                        // which covers `/tp` with no map name and every name not in the table; with its
                        // chat gate off it is a log line and nothing else.
                        self.say(
                            LogLevel::Info,
                            format!("Gameplay connection from {peer}: Unknown command"),
                        );
                    }
                }
            }
            other => {
                // `@`, `%` and `"` are ignored by the reference; anything else it refuses.
                if matches!(other, '@' | '%' | '"') {
                    self.say(
                        LogLevel::Debug,
                        format!(
                            "Gameplay connection from {peer}: channel {other:?} ignored, as the reference ignores it."
                        ),
                    );
                } else {
                    self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: chat on channel {other:?} refused, as the reference refuses it."
                        ),
                    );
                }
            }
        }
    }
}
