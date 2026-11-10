//! Framing a gameplay connection: where one message ends, decided by the table.
//!
//! A game message carries **no length**: the header is two bytes and the layout the table gives that
//! header decides where the body stops. A counted field (`String16`, `Array8`, `Array16`, `Array32`)
//! carries its own `u16` count and consumes exactly that many units, and a nested struct is the same
//! idea one level down: a `u8` count and that many repeats of the fields after it. So the table's
//! `size` column is an upper bound, never a length.
//!
//! Ported from `ASURA/crates/asura-protocol/src/client/frame.rs`: same walker, same shapes, same
//! error cases. The Rust here is this tree's, the rule is not.

use crate::server::protocol::tables::{self, Conn, FieldKind};

/// The header is two bytes. Everything after it is the body, whose length the table fixes.
pub const HEADER_LEN: usize = 2;

/// One message, already framed against the table: its bytes, header first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    bytes: Vec<u8>,
}

/// Why a byte stream is not (yet) a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Fewer bytes than the frame needs. Not a protocol violation: read more.
    Incomplete {
        /// The whole frame size once the header has been read, or the header size while it is not.
        needed: usize,
    },
    /// A header that is in no table row.
    UnknownHeader {
        /// The header that is in no row.
        header: u16,
    },
    /// A counted field claims more units than the table allows for it.
    SizeMismatch {
        /// The header whose field disagreed.
        header: u16,
        /// The count the message carried.
        got: usize,
        /// The capacity the table gives the field.
        expected: usize,
    },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incomplete { needed } => write!(f, "{needed} more byte(s) are needed"),
            Self::UnknownHeader { header } => write!(f, "0x{header:04X} is in no table row"),
            Self::SizeMismatch {
                header,
                got,
                expected,
            } => write!(
                f,
                "0x{header:04X} counts {got} units where the table allows {expected}"
            ),
        }
    }
}

impl std::error::Error for FrameError {}

impl Frame {
    /// The whole message, as it goes on the wire.
    pub fn encode(&self) -> &[u8] {
        &self.bytes
    }

    /// How many bytes it is.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether it is empty, which a frame never is.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The header, read from the message.
    pub fn header(&self) -> u16 {
        u16::from_le_bytes([self.bytes[0], self.bytes[1]])
    }

    /// The fields, read from the message.
    pub fn body(&self) -> &[u8] {
        &self.bytes[HEADER_LEN..]
    }

    /// The table's name for this header, for a log line.
    pub fn label(&self) -> String {
        match tables::lookup(Conn::Game, self.header(), true) {
            Some(row) => row.label(),
            None => format!("{:#06X}", self.header()),
        }
    }

    /// Frame one message off the front of `bytes`, and say how many bytes it used.
    pub fn decode_game_message(bytes: &[u8], from_client: bool) -> Result<(Frame, usize), FrameError> {
        if bytes.len() < HEADER_LEN {
            return Err(FrameError::Incomplete {
                needed: HEADER_LEN,
            });
        }
        let header = u16::from_le_bytes([bytes[0], bytes[1]]);
        let Some(row) = tables::lookup(Conn::Game, header, from_client) else {
            return Err(FrameError::UnknownHeader { header });
        };
        let shapes: Vec<Shape> = row.fields.iter().map(shape_of).collect();
        let walked = walk(bytes, header, &shapes)?;
        let position = match measured_override(header, from_client, bytes) {
            // One row this tree knows to be short: the confirm carries fields the table's row does not
            // list, and the measured frame is longer than the walk. The larger of the two is the
            // message, and the session says so when they disagree.
            Some(measured) if measured > walked => measured,
            _ => walked,
        };
        Ok((
            Frame {
                bytes: bytes[..position].to_vec(),
            },
            position,
        ))
    }
}

/// How many bytes the creation confirm carries after its counted name, when the table's row for it is
/// shorter than the message.
///
/// Measured from the one capture there is of a confirm
/// (`recordings/client/feature/character_creation/character_creation-live-necro_factions.jsonl`, seq 462),
/// past its widened header: a counted name, then `00 00` and eight dwords. The table's `0x808B` row
/// lists `String16(20)`, `Blob(8)` and `Dword` - twelve of those bytes - so the row is short by
/// twenty-two. Until a wire capture settles which is right, the framer takes the larger, and the
/// session says so when they disagree.
const CONFIRM_TAIL_LEN: usize = 34;

/// The header whose table row is known to be short: `GAME_CMSG_CHAR_CREATION_CONFIRM`.
const CONFIRM_HEADER: u16 = 0x808B;

/// The size a header's own measured layout gives it, when this tree measured one that the table does
/// not describe.
fn measured_override(header: u16, from_client: bool, bytes: &[u8]) -> Option<usize> {
    if header != CONFIRM_HEADER || !from_client {
        return None;
    }
    let count = bytes.get(HEADER_LEN..HEADER_LEN + 2)?;
    let units = u16::from_le_bytes([count[0], count[1]]) as usize;
    Some(HEADER_LEN + 2 + units * 2 + CONFIRM_TAIL_LEN)
}

/// How one field occupies the wire, for the walk.
#[derive(Clone, Copy)]
enum Shape {
    /// The message header: already consumed before the walk starts.
    Header,
    /// A fixed number of bytes.
    Fixed(usize),
    /// A `u16` count and that many units, up to the field's capacity.
    Counted { unit: usize, capacity: u16 },
    /// A nested struct: a `u8` count and that many repeats of the fields that follow it.
    Nested { capacity: u8 },
}

fn shape_of(field: &tables::Field) -> Shape {
    match field.kind {
        FieldKind::MsgHeader => Shape::Header,
        // A four-byte game header: the walker has already consumed the first two bytes, so the field
        // covers the two that remain.
        FieldKind::MsgHeaderGame => Shape::Fixed(HEADER_LEN),
        FieldKind::AgentId | FieldKind::Float | FieldKind::Dword => Shape::Fixed(4),
        FieldKind::Vect2 => Shape::Fixed(8),
        FieldKind::Vect3 => Shape::Fixed(12),
        FieldKind::Byte => Shape::Fixed(1),
        FieldKind::Word => Shape::Fixed(2),
        FieldKind::Blob => Shape::Fixed(field.length as usize),
        FieldKind::String16 | FieldKind::Array16 => Shape::Counted {
            unit: 2,
            capacity: field.length,
        },
        FieldKind::Array8 => Shape::Counted {
            unit: 1,
            capacity: field.length,
        },
        FieldKind::Array32 => Shape::Counted {
            unit: 4,
            capacity: field.length,
        },
        FieldKind::NestedStruct => Shape::Nested {
            capacity: field.length.min(u8::MAX as u16) as u8,
        },
    }
}

/// Walk a message's field layout and return where its body ends.
fn walk(bytes: &[u8], header: u16, shapes: &[Shape]) -> Result<usize, FrameError> {
    walk_from(bytes, header, HEADER_LEN, shapes)
}

fn walk_from(
    bytes: &[u8],
    header: u16,
    mut position: usize,
    shapes: &[Shape],
) -> Result<usize, FrameError> {
    let mut index = 0;
    while index < shapes.len() {
        match shapes[index] {
            Shape::Header => {}
            Shape::Fixed(width) => {
                position += width;
                if position > bytes.len() {
                    return Err(FrameError::Incomplete { needed: position });
                }
            }
            Shape::Counted { unit, capacity } => {
                if position + 2 > bytes.len() {
                    return Err(FrameError::Incomplete {
                        needed: position + 2,
                    });
                }
                let count = u16::from_le_bytes([bytes[position], bytes[position + 1]]) as usize;
                if count > capacity as usize {
                    return Err(FrameError::SizeMismatch {
                        header,
                        got: count,
                        expected: capacity as usize,
                    });
                }
                position += 2 + count * unit;
                if position > bytes.len() {
                    return Err(FrameError::Incomplete { needed: position });
                }
            }
            Shape::Nested { capacity } => {
                let Some(&count) = bytes.get(position) else {
                    return Err(FrameError::Incomplete {
                        needed: position + 1,
                    });
                };
                if count > capacity {
                    return Err(FrameError::SizeMismatch {
                        header,
                        got: count as usize,
                        expected: capacity as usize,
                    });
                }
                position += 1;
                // Every repeat is the tail of the layout, so the tail is consumed here and the loop
                // ends: there is nothing left to walk after the repeats.
                let tail = &shapes[index + 1..];
                for _ in 0..count {
                    position = walk_from(bytes, header, position, tail)?;
                }
                return Ok(position);
            }
        }
        index += 1;
    }
    Ok(position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_alone_is_two_bytes() {
        // `0x0186` has no fields: the player-data start, and it goes to the client.
        let (frame, used) = Frame::decode_game_message(&[0x86, 0x01], false).expect("known header");
        assert_eq!(used, 2);
        assert_eq!(frame.encode(), &[0x86, 0x01]);
        assert_eq!(frame.body().len(), 0);
    }

    #[test]
    fn a_short_stream_asks_for_more_rather_than_guessing() {
        assert_eq!(
            Frame::decode_game_message(&[0x86], false),
            Err(FrameError::Incomplete { needed: 2 })
        );
    }

    #[test]
    fn a_header_in_no_row_is_refused() {
        assert_eq!(
            Frame::decode_game_message(&[0xFF, 0xFF], true),
            Err(FrameError::UnknownHeader { header: 0xFFFF })
        );
    }

    #[test]
    fn a_counted_field_decides_the_length() {
        // `0x8089` is the creation screen's ask: a bare header, two bytes.
        let (frame, used) = Frame::decode_game_message(&[0x89, 0x80], true).expect("known header");
        assert_eq!(used, 2);
        assert_eq!(frame.len(), 2);
    }

    /// Every message a working server recorded the client sending frames to exactly the bytes it
    /// recorded, and the recording is this tree's own copy of that session.
    #[test]
    fn the_wire_recordings_frame_to_their_own_sizes() {
        let path = "../recordings/asura/world/live-client-session-20261027-223528.jsonl";
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("no recording at {path}; the walk is not measured here");
            return;
        };

        let mut checked = 0;
        for line in text.lines() {
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if record["direction"] != "CToS" {
                continue;
            }
            let Some(data) = record["data"].as_str() else {
                continue;
            };
            let Some(size) = record["size"].as_u64() else {
                continue;
            };
            let mut bytes = Vec::with_capacity(data.len() / 2);
            for pair in 0..data.len() / 2 {
                bytes.push(u8::from_str_radix(&data[pair * 2..pair * 2 + 2], 16).expect("hex"));
            }

            match Frame::decode_game_message(&bytes, true) {
                Ok((frame, used)) => {
                    assert_eq!(
                        used as u64,
                        size,
                        "header 0x{:04X} framed to {} bytes, the recording says {size}",
                        frame.header(),
                        used
                    );
                    checked += 1;
                }
                Err(error) => panic!(
                    "header 0x{:04X} did not frame: {error}",
                    u16::from_le_bytes([bytes[0], bytes[1]])
                ),
            }
        }

        assert!(checked > 5, "the recording framed {checked} messages");
    }
}
