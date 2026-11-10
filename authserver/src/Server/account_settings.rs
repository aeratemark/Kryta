//! The account settings block: the 591 bytes a client is handed on every login.
//!
//! A value file, `data/account_settings.json`, because it is a copied payload rather than something
//! this server constructs. Most of it is not understood, and keeping it byte for byte is the only way
//! to hand the client what it expects: a wrong length here is a desynchronised connection, because the
//! client reads the next message's header out of the middle of this one.

use common::configuration::Config;

/// Read the block.
///
/// The file states it as one byte string under `data`, written the way a capture prints it. A file
/// whose length is not the one the records give is refused rather than sent: a block of the wrong
/// size is worse than no block, because the client cannot recover from it.
pub fn load(path: impl AsRef<std::path::Path>) -> Result<Vec<u8>, String> {
    let path = path.as_ref();
    let config = Config::load_json(path, false, Default::default())
        .map_err(|error| error.to_string())?;

    let bytes = match config.bytes("data") {
        Some(Ok(bytes)) => bytes,
        Some(Err(reason)) => return Err(format!("{}: {reason}", path.display())),
        None => {
            // A value the reader refused is dropped with a complaint, so the complaint is what says
            // what was wrong with it; with none, the file simply does not state the key.
            if config.complaints().is_empty() {
                return Err(format!("{}: no 'data' byte string", path.display()));
            }
            let reason = config
                .complaints()
                .iter()
                .map(|complaint| complaint.message.trim_start_matches("> Config: ").to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!("{}: {reason}", path.display()));
        }
    };

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_block_is_five_hundred_and_ninety_one_bytes() {
        let settings = load("../data/account_settings.json").expect("the shipped block");
        assert_eq!(settings.len(), 591);
        // The first bytes, which a capture shows and a transposition would move.
        assert_eq!(&settings[..4], &[0x08, 0x00, 0x66, 0x21]);
    }

    #[test]
    fn a_file_with_no_data_is_refused() {
        let dir = std::env::temp_dir().join(format!("kryta-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.json");
        std::fs::write(&path, r#"{"note": "nothing here"}"#).unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn a_word_that_is_not_hexadecimal_is_refused_with_its_text() {
        let dir = std::env::temp_dir().join(format!("kryta-settings-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        std::fs::write(&path, r#"{"data": {"hex": "08 00 zz"}}"#).unwrap();
        let error = load(&path).unwrap_err();
        assert!(error.contains("'zz'"), "{error}");
    }
}
