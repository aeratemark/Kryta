//! The chat message family: a line spoken, and who spoke it.
//!
//! Reference: `src/server/game/Server/Packets/ChatPackets.{h,cpp}` - one file pair per message family,
//! and this is the family that carries chat. Nothing of its *contents* crosses: these headers, these
//! layouts and these channel numbers are this client's, ported from the working implementation's own
//! builders (`ASURA/crates/asura-protocol/src/game/mod.rs:236-274`).
//!
//! None of the three appears in any capture, in either direction: both servers' chat has only ever been
//! printed, and no chat text has ever reached a client's screen
//! (`ASURA/docs/reference/client-protocol.md:920-937`). What is written here is therefore the table's
//! own field list - `String16` for `0x005D`, two dwords for each of the other two - which is what the
//! working implementation wrote and what its own length test asserts (10 bytes for `0x0061`).

/// `GAME_SMSG_CHAT_MESSAGE_CORE` (`0x005D`): the text of one chat message, `2 + 2 + 2n`.
pub const CHAT_MESSAGE_CORE: u16 = 0x005D;
/// `GAME_SMSG_CHAT_MESSAGE_SERVER` (`0x005E`): 10 bytes - the server itself speaking on a channel.
pub const CHAT_MESSAGE_SERVER: u16 = 0x005E;
/// `GAME_SMSG_CHAT_MESSAGE_LOCAL` (`0x0061`): 10 bytes - which player spoke, on which channel.
pub const CHAT_MESSAGE_LOCAL: u16 = 0x0061;

/// How many UTF-16 units `0x005D` carries at most.
///
/// The table declares its `String16` field 122 units long (`ASURA/crates/asura-protocol/src/packets.rs`,
/// header `0x005D`), and the working implementation's builder caps the text at exactly that
/// (`chat_message_core`, which passes `122` to its text writer).
pub const CHAT_MESSAGE_CORE_UNITS: usize = 122;

/// `Channel_All`, in the client's own enum (`GmChat.h`): the channel `!` speaks on.
pub const CHANNEL_ALL: u32 = 3;
/// `Channel_Group`: the channel `#` speaks on.
pub const CHANNEL_GROUP: u32 = 11;
/// `Channel_Trade`: the channel `$` speaks on.
pub const CHANNEL_TRADE: u32 = 12;
/// `Channel_Warning`: the middle-of-the-screen channel the reference answers a command on.
///
/// Nothing here sends it: every answer the reference generates is printed rather than sent
/// (`ASURA/crates/asura-world/src/chat.rs:288-352`), so the constant arrives with its family and waits
/// for the answer that would use it.
pub const CHANNEL_WARNING: u32 = 7;

/// A frame: the `u16` header and then the body.
fn frame(header: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A `u16` count followed by that many UTF-16 code units, which is the wire's `String16`.
fn units_field(units: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + units.len() * 2);
    out.extend_from_slice(&(units.len() as u16).to_le_bytes());
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

/// `0x005D`: the text of one chat message.
///
/// `ASURA/crates/asura-protocol/src/game/mod.rs:253`, `chat_message_core`: a `u16` count and the units,
/// with the text capped at the field's declared length and no padding after it.
pub fn chat_message_core(text: &str) -> Vec<u8> {
    let units: Vec<u16> = text
        .encode_utf16()
        .take(CHAT_MESSAGE_CORE_UNITS)
        .collect();
    frame(CHAT_MESSAGE_CORE, &units_field(&units))
}

/// `0x0061`: who is speaking, and on which channel.
///
/// `ASURA/crates/asura-protocol/src/game/mod.rs:258`, `chat_message_local`: the player and the channel,
/// ten bytes on the wire - the length its own test asserts.
pub fn chat_message_local(player: u32, channel: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&player.to_le_bytes());
    body.extend_from_slice(&channel.to_le_bytes());
    frame(CHAT_MESSAGE_LOCAL, &body)
}

/// `0x005E`: the server itself speaking on a channel.
///
/// `ASURA/crates/asura-protocol/src/game/mod.rs:269`, `chat_message_server`: the channel and a zero
/// dword. The second dword is not in the reference's struct and is zero on the wire, but the table
/// declares the message at 10 bytes and a client given 6 reads two bytes of the next message as part of
/// this one - which is why the working implementation writes it.
pub fn chat_message_server(channel: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&channel.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    frame(CHAT_MESSAGE_SERVER, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_is_a_count_and_its_units() {
        // `Race` is four units: `2 + 2 + 8`.
        assert_eq!(
            chat_message_core("Race"),
            vec![0x5D, 0x00, 4, 0, b'R', 0, b'a', 0, b'c', 0, b'e', 0]
        );
        assert_eq!(chat_message_core("Race").len(), 12);
    }

    #[test]
    fn the_text_stops_at_the_field_it_is_declared_in() {
        let long = "x".repeat(CHAT_MESSAGE_CORE_UNITS + 8);
        let frame = chat_message_core(&long);
        // Two header bytes, the count, and 122 units - the field's own length.
        assert_eq!(frame.len(), 2 + 2 + CHAT_MESSAGE_CORE_UNITS * 2);
        assert_eq!(
            u16::from_le_bytes([frame[2], frame[3]]) as usize,
            CHAT_MESSAGE_CORE_UNITS
        );
    }

    #[test]
    fn a_local_line_is_the_player_and_the_channel() {
        assert_eq!(
            chat_message_local(1, CHANNEL_ALL),
            vec![0x61, 0x00, 1, 0, 0, 0, 3, 0, 0, 0]
        );
        assert_eq!(chat_message_local(1, CHANNEL_ALL).len(), 10);
        assert_eq!(chat_message_local(1, CHANNEL_GROUP).len(), 10);
    }

    #[test]
    fn the_server_speaks_in_ten_bytes() {
        assert_eq!(
            chat_message_server(CHANNEL_WARNING),
            vec![0x5E, 0x00, 7, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(chat_message_server(CHANNEL_WARNING).len(), 10);
    }

    /// The channel numbers are the client's own (`GmChat.h`, as the working implementation records
    /// them): three tests there pin all three (`ASURA/crates/asura-world/src/lib.rs:989-992`).
    #[test]
    fn the_channels_are_the_clients_own_numbers() {
        assert_eq!(CHANNEL_ALL, 3);
        assert_eq!(CHANNEL_GROUP, 11);
        assert_eq!(CHANNEL_TRADE, 12);
    }
}
