//! The game connection's protocol: the opcodes and the messages this tree has measured.
//!
//! Reference: `src/server/game/Server/Protocol/`, which holds the opcode table (`Opcodes.{h,cpp}`),
//! the packet header and the packet log. Nothing of its *contents* crosses: Azeroth's opcodes are
//! not this client's. What crosses is the arrangement - one table, one header, and a message type
//! per family - and it fills in as this client's messages are measured
//! (`docs/port/02-wire.md`).
//!
//! Here now:
//!
//! - [`tables`] - the whole client protocol, ported: 782 rows of header, direction, index, declared
//!   size, name and field layout, both families. It is the table a session asked for
//!   (`tools/adapt-protocol-table.py` records how it was moved into place).
//! - `GAME_CMSG_VERSION`, the 64-byte message a client sends when it opens its gameplay connection.
//!   It is the message the front door reads to learn which world a client was sent to, and the first
//!   one the world program reads itself (`docs/decisions/0004`).

pub use shared::packets::frame;
pub use shared::packets::tables;
pub use shared::packets::{Frame, FrameError, HEADER_LEN};

/// The four bytes that name a gameplay connection, as a bare little-endian `u32`.
///
/// Evidence: `docs/evidence/client-protocol.md` section 2 - a capture of this client's three
/// connection headers, this one with a 64-byte first message.
pub const GAME_CMSG_VERSION: u32 = 0x000C_0500;

/// How many bytes the message is.
///
/// Evidence: the same section, and the reference's own packed struct: a four-byte header, five
/// dwords, two 16-byte identifiers and two trailing dwords.
pub const GAME_CMSG_VERSION_LEN: usize = 64;

/// `GAME_CMSG_VERSION`: what a client says when it opens its gameplay connection.
///
/// Layout, in the order the reference's `Connection_ReadVersion` reads it:
///
/// | off | size | field |
/// |---|---|---|
/// | 0 | 4 | the header, `0x000C0500` |
/// | 4 | 4 | the client's build number, never compared |
/// | 8 | 4 | `h0008`, observed 1 |
/// | 12 | 4 | `map_token` - the world's id, from the answer to `0x8029` |
/// | 16 | 4 | `map_id` - the map that answer named |
/// | 20 | 4 | `player_token` - drawn per travel by the login |
/// | 24 | 16 | the account, as a little-endian UUID |
/// | 40 | 16 | the character, the same |
/// | 56 | 4 | `h0038`, observed 0 |
/// | 60 | 4 | `h003C`, observed 0 |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameVersion {
    /// The client's build number.
    pub version: u32,
    /// `h0008`, observed 1 and never compared.
    pub h0008: u32,
    /// The world the client was sent to, as the login numbered it.
    pub map_token: u32,
    /// The map it was told about.
    pub map_id: u32,
    /// The number the login drew for this travel.
    pub player_token: u32,
    /// The account, in the wire's little-endian UUID form.
    pub account: [u8; 16],
    /// The character, the same form.
    pub character: [u8; 16],
    /// `h0038`, observed 0.
    pub h0038: u32,
    /// `h003C`, observed 0.
    pub h003c: u32,
}

/// Why a `GAME_CMSG_VERSION` could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameVersionError {
    /// Fewer than 64 bytes: the caller reads more rather than guessing where the seed begins.
    Incomplete {
        /// How many bytes are still needed.
        needed: usize,
    },
    /// The first four bytes are another connection's.
    NotAGameVersion {
        /// What they are instead.
        header: u32,
    },
}

impl std::fmt::Display for GameVersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incomplete { needed } => write!(f, "{needed} more byte(s) are needed"),
            Self::NotAGameVersion { header } => {
                write!(f, "0x{header:08X} is not a gameplay connection")
            }
        }
    }
}

impl std::error::Error for GameVersionError {}

impl GameVersion {
    /// Read the message off the front of `bytes`, and say how many bytes it used.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), GameVersionError> {
        if bytes.len() < GAME_CMSG_VERSION_LEN {
            return Err(GameVersionError::Incomplete {
                needed: GAME_CMSG_VERSION_LEN - bytes.len(),
            });
        }

        let dword = |offset: usize| {
            u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ])
        };

        let header = dword(0);
        if header != GAME_CMSG_VERSION {
            return Err(GameVersionError::NotAGameVersion { header });
        }

        let mut account = [0u8; 16];
        account.copy_from_slice(&bytes[24..40]);
        let mut character = [0u8; 16];
        character.copy_from_slice(&bytes[40..56]);

        Ok((
            Self {
                version: dword(4),
                h0008: dword(8),
                map_token: dword(12),
                map_id: dword(16),
                player_token: dword(20),
                account,
                character,
                h0038: dword(56),
                h003c: dword(60),
            },
            GAME_CMSG_VERSION_LEN,
        ))
    }

    /// The 64 bytes a client sends. Written here so a test can play the client.
    pub fn encode(&self) -> [u8; GAME_CMSG_VERSION_LEN] {
        let mut bytes = [0u8; GAME_CMSG_VERSION_LEN];
        for (offset, value) in [
            (0, GAME_CMSG_VERSION),
            (4, self.version),
            (8, self.h0008),
            (12, self.map_token),
            (16, self.map_id),
            (20, self.player_token),
            (56, self.h0038),
            (60, self.h003c),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[24..40].copy_from_slice(&self.account);
        bytes[40..56].copy_from_slice(&self.character);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GameVersion {
        GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 2,
            map_id: 0,
            player_token: 0xA2E3_DB8C,
            account: [0xAA; 16],
            character: [0xBB; 16],
            h0038: 0,
            h003c: 0,
        }
    }

    #[test]
    fn the_message_is_sixty_four_bytes_and_round_trips() {
        let version = sample();
        let bytes = version.encode();
        assert_eq!(bytes.len(), 64);
        assert_eq!(GameVersion::decode(&bytes), Ok((version, 64)));
    }

    #[test]
    fn every_field_is_at_the_offset_the_reference_reads_it() {
        // Spelled out so a reordering fails a test rather than a client.
        let bytes = sample().encode();
        assert_eq!(&bytes[0..4], &0x000C_0500u32.to_le_bytes(), "the header");
        assert_eq!(&bytes[4..8], &39167u32.to_le_bytes(), "the build");
        assert_eq!(&bytes[8..12], &1u32.to_le_bytes(), "h0008");
        assert_eq!(&bytes[12..16], &2u32.to_le_bytes(), "map_token");
        assert_eq!(&bytes[16..20], &0u32.to_le_bytes(), "map_id");
        assert_eq!(&bytes[20..24], &0xA2E3_DB8Cu32.to_le_bytes(), "player_token");
        assert_eq!(&bytes[24..40], &[0xAA; 16], "the account");
        assert_eq!(&bytes[40..56], &[0xBB; 16], "the character");
        assert_eq!(&bytes[56..64], &[0u8; 8], "h0038 and h003C");
    }

    #[test]
    fn a_short_message_asks_for_more_rather_than_reading_garbage() {
        let bytes = sample().encode();
        for cut in [0usize, 1, 4, 24, 63] {
            assert_eq!(
                GameVersion::decode(&bytes[..cut]),
                Err(GameVersionError::Incomplete {
                    needed: 64 - cut
                }),
                "cut at {cut}"
            );
        }
    }

    #[test]
    fn a_login_connection_is_not_a_gameplay_connection() {
        let mut bytes = [0u8; 64];
        bytes[0..4].copy_from_slice(&0x000C_0400u32.to_le_bytes());
        assert!(matches!(
            GameVersion::decode(&bytes),
            Err(GameVersionError::NotAGameVersion { header }) if header == 0x000C_0400
        ));
    }
}
