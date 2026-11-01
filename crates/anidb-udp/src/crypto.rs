//! AES-128-ECB session encryption (the ENCRYPT command). The specification
//! describes the command, see the crate root for the link.

use aes::Aes128;
use ecb::cipher::{block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyInit};
use md5::{Digest, Md5};

use crate::error::Error;

/// A session key, derived from the API key and the salt of the ENCRYPT reply.
#[derive(Clone, PartialEq, Eq)]
pub struct Key([u8; 16]);

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(<redacted>)")
    }
}

impl Key {
    /// The key is the md5 hash of the UDP API key (set on the profile settings
    /// page, not the password) followed by the salt from the reply to ENCRYPT.
    #[must_use]
    pub fn new(api_key: &str, salt: &str) -> Self {
        let digest = Md5::new()
            .chain_update(api_key.as_bytes())
            .chain_update(salt.as_bytes())
            .finalize();
        Self(digest.into())
    }

    /// Encrypts a request with AES-128-ECB and PKCS#7 padding. Once encryption
    /// is on, every request must be encrypted, the server discards plain ones.
    #[must_use]
    pub fn encrypt(&self, plaintext: &[u8]) -> Vec<u8> {
        let key = self.0;
        ecb::Encryptor::<Aes128>::new(&key.into()).encrypt_padded_vec::<Pkcs7>(plaintext)
    }

    /// Decrypts a reply. Everything after the reply to ENCRYPT is encrypted
    /// until LOGOUT or a session timeout turns encryption off.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Decrypt`] when the ciphertext is not a whole number of blocks or its padding is wrong.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, Error> {
        let key = self.0;
        ecb::Decryptor::<Aes128>::new(&key.into())
            .decrypt_padded_vec::<Pkcs7>(ciphertext)
            .map_err(|_| Error::Decrypt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    // md5 of "akibaKey" followed by "1a2b3c4d", computed with an independent tool
    #[test]
    fn key_is_md5_of_api_key_and_salt() {
        let key = Key::new("akibaKey", "1a2b3c4d");
        assert_eq!(key.0.to_vec(), hex("5facb035ace0dc36e44a8b4e77c31ac8"));
    }

    // aes-128-ecb with PKCS#5 padding and the key above, computed with an independent tool
    #[test]
    fn encrypt_matches_reference_vector() {
        let key = Key::new("akibaKey", "1a2b3c4d");
        let ciphertext = key.encrypt(b"AUTH user=x&tag=ABCDE");
        assert_eq!(
            ciphertext,
            hex("c24bf2ec87f5f3c951a55a019432302c87eaa2e1a611afec34037d6d03d83205")
        );
        assert_eq!(key.decrypt(&ciphertext).unwrap(), b"AUTH user=x&tag=ABCDE");
    }

    #[test]
    fn round_trip_for_every_length_around_block_size() {
        let key = Key::new("api", "salt");
        for len in 0..40 {
            let plaintext: Vec<u8> = (0..len).map(|i| b'a' + (i % 26)).collect();
            let ciphertext = key.encrypt(&plaintext);
            assert_eq!(ciphertext.len() % 16, 0);
            assert!(ciphertext.len() > plaintext.len());
            assert_eq!(key.decrypt(&ciphertext).unwrap(), plaintext, "len {len}");
        }
    }

    #[test]
    fn decrypt_rejects_bad_length() {
        let key = Key::new("api", "salt");
        assert!(matches!(key.decrypt(&[0u8; 15]), Err(Error::Decrypt)));
        assert!(matches!(key.decrypt(&[]), Err(Error::Decrypt)));
    }

    #[test]
    fn debug_hides_the_key() {
        let key = Key::new("api", "salt");
        assert_eq!(format!("{key:?}"), "Key(<redacted>)");
    }
}
