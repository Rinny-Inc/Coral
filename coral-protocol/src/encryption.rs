use aes::Aes128;
use cfb8::{Decryptor, Encryptor};
use cipher::KeyIvInit;
use rand::{Rng, rngs::SysRng};
use rsa::{Pkcs1v15Encrypt, PrivateKey, PublicKey, pkcs8::EncodePublicKey, rand_core::UnwrapErr};

pub type Aes128Cfb8Enc = Encryptor<Aes128>;
pub type Aes128Cfb8Dec = Decryptor<Aes128>;

pub const SHARED_SECRET_LEN: usize = 16;
pub const VERIFY_TOKEN_LEN: usize = 4;

pub struct Encryption {
    pub encrypt: Aes128Cfb8Enc,
    pub decrypt: Aes128Cfb8Dec,
}
impl Encryption {
    pub fn new(key: &[u8; SHARED_SECRET_LEN]) -> Self {
        Encryption {
            encrypt: Aes128Cfb8Enc::new(key.into(), key.into()),
            decrypt: Aes128Cfb8Dec::new(key.into(), key.into()),
        }
    }

    pub fn encrypt(&mut self, data: &mut [u8]) {
        self.encrypt.encrypt(data);
    }
    pub fn decrypt(&mut self, data: &mut [u8]) {
        self.decrypt.decrypt(data);
    }
}

pub fn generate_rsa_key() -> (PrivateKey, Vec<u8>) {
    let mut rng = UnwrapErr(SysRng);
    let private_key = PrivateKey::new(&mut rng, 1024).unwrap();
    let public_key = PublicKey::from(&private_key);
    let der = public_key.to_public_key_der().unwrap();
    (private_key, der.to_vec())
}

pub fn decrypt_rsa(private_key: &PrivateKey, data: &[u8]) -> Option<Vec<u8>> {
    private_key.decrypt(Pkcs1v15Encrypt, data).ok()
}

pub fn decrypt_shared_secret(
    private_key: &PrivateKey,
    data: &[u8],
) -> Option<[u8; SHARED_SECRET_LEN]> {
    decrypt_rsa(private_key, data)?.try_into().ok()
}

pub fn tokens_match(decrypted: &[u8], expected: &[u8]) -> bool {
    if decrypted.len() != expected.len() {
        return false;
    }
    decrypted
        .iter()
        .zip(expected)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

pub fn generate_verify_token() -> Vec<u8> {
    let mut token = vec![0u8; VERIFY_TOKEN_LEN];
    UnwrapErr(SysRng).fill_bytes(&mut token);
    token
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keypair() -> (PrivateKey, PublicKey) {
        let (private_key, _) = generate_rsa_key();
        let public_key = PublicKey::from(&private_key);
        (private_key, public_key)
    }

    fn encrypt_for(public_key: &PublicKey, plain: &[u8]) -> Vec<u8> {
        public_key
            .encrypt(&mut UnwrapErr(SysRng), Pkcs1v15Encrypt, plain)
            .expect("encrypt")
    }

    #[test]
    fn a_shared_secret_survives_the_round_trip() {
        let (private_key, public_key) = keypair();
        let secret = [0x5Au8; SHARED_SECRET_LEN];

        let ct = encrypt_for(&public_key, &secret);
        assert_eq!(decrypt_shared_secret(&private_key, &ct), Some(secret));
    }

    #[test]
    fn a_corrupt_ciphertext_decrypts_to_noise_rather_than_failing() {
        let (private_key, public_key) = keypair();
        let mut ct = encrypt_for(&public_key, &[0x5Au8; SHARED_SECRET_LEN]);
        ct[5] ^= 0xFF;

        let raw = decrypt_rsa(&private_key, &ct)
            .expect("implicit rejection reports success on bad padding");
        assert_ne!(
            raw.len(),
            SHARED_SECRET_LEN,
            "synthetic plaintext is modulus-length, not key-length"
        );

        assert_eq!(
            decrypt_shared_secret(&private_key, &ct),
            None,
            "noise must not be accepted as an AES key"
        );
    }

    #[test]
    fn a_wrong_sized_ciphertext_is_rejected() {
        let (private_key, _) = keypair();
        assert_eq!(decrypt_shared_secret(&private_key, &[0u8; 8]), None);
        assert_eq!(decrypt_rsa(&private_key, &[]), None);
    }

    #[test]
    fn a_verify_token_round_trips_and_a_forged_one_does_not() {
        let (private_key, public_key) = keypair();
        let token = generate_verify_token();
        assert_eq!(token.len(), VERIFY_TOKEN_LEN);

        let ct = encrypt_for(&public_key, &token);
        let decrypted = decrypt_rsa(&private_key, &ct).expect("decrypt");
        assert!(tokens_match(&decrypted, &token));

        let mut bad = ct.clone();
        bad[3] ^= 0xFF;
        let noise = decrypt_rsa(&private_key, &bad).expect("implicit rejection");
        assert!(
            !tokens_match(&noise, &token),
            "noise must not pass as the token"
        );
    }

    #[test]
    fn token_comparison_rejects_length_mismatches_and_single_bit_flips() {
        assert!(tokens_match(&[1, 2, 3, 4], &[1, 2, 3, 4]));
        assert!(!tokens_match(&[1, 2, 3, 4], &[1, 2, 3, 5]));
        assert!(!tokens_match(&[1, 2, 3], &[1, 2, 3, 4]));
        assert!(!tokens_match(&[], &[1]));
        assert!(tokens_match(&[], &[]));
    }

    #[test]
    fn the_stream_cipher_round_trips() {
        let key = [0x11u8; SHARED_SECRET_LEN];
        let mut out = Encryption::new(&key);
        let mut back = Encryption::new(&key);

        let mut buf = *b"handshake payload";
        let original = buf;
        out.encrypt(&mut buf);
        assert_ne!(buf, original);
        back.decrypt(&mut buf);
        assert_eq!(buf, original);
    }
}
