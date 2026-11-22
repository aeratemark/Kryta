//! The title message family: the rank table and the title tracks a load sends.
//!
//! Reference for the modern shape: `Rurik/studies/newopcodes/FINDINGS.md:1463-1477` and
//! `Rurik/studies/character/RUNS.md:188-202`. The client renders a track from the rank row it
//! references, so every rank row must precede every track that names it. The two `string16(8)`
//! fields in `0x00F3` and `0x00F6` admit at most seven encoded units on receive; a literal with
//! template framing spends three of those, leaving four text units. A longer name belongs on an
//! archive string id, not in a literal (`Rurik/studies/character/RUNS.md:165-186`).
//!
//! The 2022 client used the sibling numbering 242/243/244/245 and an eight-dword track; this
//! tree's client is the modern one, so the headers and the nine payload dwords are the modern
//! ones. The extra word at index 4 is the mode word; this platform sends zero.

/// `GAME_SMSG_TITLE_RANK_DATA` (`0x00F3`): one rank definition.
pub const TITLE_RANK_DATA: u16 = 0x00F3;
/// `GAME_SMSG_TITLE_RANK_DISPLAY` (`0x00F4`): the rank one player displays.
pub const TITLE_RANK_DISPLAY: u16 = 0x00F4;
/// `GAME_SMSG_TITLE_UPDATE` (`0x00F5`): a fast points patch.
pub const TITLE_UPDATE: u16 = 0x00F5;
/// `GAME_SMSG_TITLE_TRACK_INFO` (`0x00F6`): one title track.
pub const TITLE_TRACK_INFO: u16 = 0x00F6;

/// The literal-run marker and the template id, measured in Rurik's quest/title text work.
const TEMPLATE_STR1: u16 = 0x0BA9;
const LITERAL_MARK: u16 = 0x0107;
const LITERAL_END: u16 = 0x0001;
/// The bare string id the client accepts as an empty rank name (retail record `0x0101`).
const EMPTY_NAME: u16 = 0x0101;

/// A `u16` count followed by that many UTF-16 code units, the wire's `String16`.
fn units_field(units: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + units.len() * 2);
    out.extend_from_slice(&(units.len() as u16).to_le_bytes());
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

/// A frame: the `u16` header and then the body.
fn frame(header: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A short server-authored title string as the client's coded-string field.
///
/// An empty name is the bare string id `0x0101`, which is the retail rank-zero spelling. A
/// non-empty name is template framing, the literal, and the terminator. The caller's text is
/// limited to four UTF-16 units because the field is `string16(8)` and framing spends three.
pub(crate) fn coded_title_name(text: &str) -> Vec<u16> {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.is_empty() {
        return vec![EMPTY_NAME];
    }
    assert!(
        units.len() <= 4,
        "a title literal must fit the 8-unit field after 3 framing units"
    );
    let mut out = Vec::with_capacity(units.len() + 3);
    out.push(TEMPLATE_STR1);
    out.push(LITERAL_MARK);
    out.extend(units);
    out.push(LITERAL_END);
    out
}

/// `0x00F3`: rank id, the unused dword, the rank number, and the coded name.
pub(crate) fn rank_data(rank_id: u32, flags: u32, rank: u32, name: &[u16]) -> Vec<u8> {
    let mut body = Vec::with_capacity(12 + 2 + name.len() * 2);
    body.extend_from_slice(&rank_id.to_le_bytes());
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&rank.to_le_bytes());
    body.extend_from_slice(&units_field(name));
    frame(TITLE_RANK_DATA, &body)
}

/// `0x00F4`: the player number and the rank id that player displays.
///
/// Rank zero clears the player's displayed title.
pub(crate) fn rank_display(player: u16, rank_id: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(6);
    body.extend_from_slice(&player.to_le_bytes());
    body.extend_from_slice(&rank_id.to_le_bytes());
    frame(TITLE_RANK_DISPLAY, &body)
}

/// `0x00F5`: a title id and its new current points.
///
/// Reserved for a metric source that updates a title after the load.
#[allow(dead_code)]
pub(crate) fn title_update(title_id: u32, points: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&title_id.to_le_bytes());
    body.extend_from_slice(&points.to_le_bytes());
    frame(TITLE_UPDATE, &body)
}

/// `0x00F6`: one track. The nine payload dwords are, in order, flags, points, current rank id,
/// current minimum, the mode word, next rank id, next minimum, max rank, max rank id; then the
/// two coded strings. This is the exact shape Rurik's live probe sent as `[7,0,points,1,min,0,2,next,2,2]`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn track_info(
    title_id: u32,
    flags: u32,
    points: u32,
    current_rank: u32,
    current_minimum: u32,
    mode: u32,
    next_rank: u32,
    next_minimum: u32,
    max_rank: u32,
    max_rank_id: u32,
    progress: &[u16],
    name: &[u16],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(40 + 4 + progress.len() * 2 + name.len() * 2);
    body.extend_from_slice(&title_id.to_le_bytes());
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&points.to_le_bytes());
    body.extend_from_slice(&current_rank.to_le_bytes());
    body.extend_from_slice(&current_minimum.to_le_bytes());
    body.extend_from_slice(&mode.to_le_bytes());
    body.extend_from_slice(&next_rank.to_le_bytes());
    body.extend_from_slice(&next_minimum.to_le_bytes());
    body.extend_from_slice(&max_rank.to_le_bytes());
    body.extend_from_slice(&max_rank_id.to_le_bytes());
    body.extend_from_slice(&units_field(progress));
    body.extend_from_slice(&units_field(name));
    frame(TITLE_TRACK_INFO, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_rank_name_is_the_bare_string_id() {
        assert_eq!(coded_title_name(""), vec![0x0101]);
    }

    #[test]
    fn a_four_unit_name_is_template_framed() {
        assert_eq!(
            coded_title_name("Jump"),
            vec![
                0x0BA9,
                0x0107,
                b'J' as u16,
                b'u' as u16,
                b'm' as u16,
                b'p' as u16,
                1
            ]
        );
    }

    #[test]
    fn a_rank_row_is_the_header_three_dwords_and_a_string() {
        let frame = rank_data(1, 0, 0, &[0x0101]);
        assert_eq!(frame.len(), 2 + 12 + 2 + 2);
        assert_eq!(&frame[..2], &TITLE_RANK_DATA.to_le_bytes());
    }

    #[test]
    fn a_track_row_is_the_header_ten_dwords_and_two_strings() {
        let frame = track_info(
            7,
            0,
            0,
            1,
            0,
            0,
            2,
            10_000,
            2,
            2,
            &coded_title_name("Pts"),
            &coded_title_name("Jump"),
        );
        assert_eq!(frame.len(), 2 + 40 + (2 + 12) + (2 + 14));
        assert_eq!(&frame[..2], &TITLE_TRACK_INFO.to_le_bytes());
        assert_eq!(&frame[2..6], &7u32.to_le_bytes());
        assert_eq!(&frame[6..10], &0u32.to_le_bytes());
        assert_eq!(&frame[34..38], &2u32.to_le_bytes());
    }

    #[test]
    fn a_display_row_is_the_header_a_word_and_a_dword() {
        assert_eq!(rank_display(1, 1), vec![0xF4, 0x00, 1, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn a_fast_update_is_the_header_and_two_dwords() {
        assert_eq!(title_update(7, 0), vec![0xF5, 0x00, 7, 0, 0, 0, 0, 0, 0, 0]);
    }
}
