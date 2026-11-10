//! `AuthCrypt`: one key, one RC4 per direction, for a connection that has finished the exchange.
//!
//! Reference: `src/server/shared/Cryptography/AuthCrypt.{h,cpp}`, which is what a socket holds after
//! the seed exchange: the reference's `EncryptSend` and `DecryptRecv` run one cipher each, and the
//! position carries across messages because it is one stream and not one call.
//!
//! Both conversations use it - the login connection and the gameplay connection - and they use it
//! identically, which is why it lives here rather than in either program.

use common::cryptography::Arc4;

use super::handshake::KEY_LEN;

/// The two directions of one conversation.
pub struct AuthCrypt {
    /// What the server sends, in stream order.
    sending: Arc4,
    /// What the server receives, in stream order.
    receiving: Arc4,
}

impl AuthCrypt {
    /// Key both directions with the digest the exchange produced.
    ///
    /// Both start at position zero: the client keys its own two instances the same way, and a stream
    /// that started anywhere else would decrypt to noise.
    pub fn new(key: &[u8; KEY_LEN]) -> Self {
        Self {
            sending: Arc4::new(key),
            receiving: Arc4::new(key),
        }
    }

    /// Scramble bytes about to be sent. The position advances by their length.
    pub fn encrypt(&mut self, bytes: &mut [u8]) {
        self.sending.apply(bytes);
    }

    /// Unscramble bytes just received. The position advances by their length.
    pub fn decrypt(&mut self, bytes: &mut [u8]) {
        self.receiving.apply(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_key_both_ways_reads_a_message_back() {
        // Both directions are keyed with the same digest and both start at position zero, so a
        // message scrambled by one is readable through the other - which is what makes the two
        // clients' streams line up.
        let key = [0x11u8; KEY_LEN];
        let mut crypt = AuthCrypt::new(&key);

        let mut sent = *b"one message";
        crypt.encrypt(&mut sent);
        assert_ne!(&sent, b"one message", "it is scrambled on the way out");

        let mut read_back = sent;
        crypt.decrypt(&mut read_back);
        assert_eq!(&read_back, b"one message");
    }

    #[test]
    fn the_position_carries_across_calls_and_is_kept_per_direction() {
        // One keystream per direction, not one per message: two writes in a row are two slices of
        // the same stream, and reading them back in the wrong order reads noise.
        let key = [0x22u8; KEY_LEN];
        let mut writer = AuthCrypt::new(&key);
        let mut first = *b"aa";
        writer.encrypt(&mut first);

        let mut second = *b"bb";
        writer.encrypt(&mut second);

        let mut reader = AuthCrypt::new(&key);
        let mut one = first;
        let mut two = second;
        reader.decrypt(&mut one);
        reader.decrypt(&mut two);
        assert_eq!(&one, b"aa");
        assert_eq!(&two, b"bb");

        // The same two slices, read in the other order: the stream is not associative.
        let mut out_of_order = AuthCrypt::new(&key);
        let mut tail = second;
        out_of_order.decrypt(&mut tail);
        assert_ne!(&tail, b"bb");
    }
}
