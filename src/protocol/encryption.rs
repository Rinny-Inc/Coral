use aes::Aes128;
use cfb8::{Decryptor, Encryptor};
use cipher::KeyIvInit;
use rsa::{
    Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
    pkcs8::EncodePublicKey,
    rand_core::{OsRng, RngCore},
};

pub type Aes128Cfb8Enc = Encryptor<Aes128>;
pub type Aes128Cfb8Dec = Decryptor<Aes128>;

pub struct Encryption {
    pub encrypt: Aes128Cfb8Enc,
    pub decrypt: Aes128Cfb8Dec,
}
impl Encryption {
    pub fn new(key: &[u8]) -> Self {
        assert_eq!(
            key.len(),
            16,
            "AES-128 key must be exactly 16 bytes, got {}",
            key.len()
        );
        Encryption {
            encrypt: Aes128Cfb8Enc::new_from_slices(key, key).unwrap(),
            decrypt: Aes128Cfb8Dec::new_from_slices(key, key).unwrap(),
        }
    }

    pub fn encrypt(&mut self, data: &mut Vec<u8>) {
        self.encrypt.encrypt(data);
    }
    pub fn decrypt(&mut self, data: &mut Vec<u8>) {
        self.decrypt.decrypt(data);
    }
}

pub fn generate_rsa_key() -> (RsaPrivateKey, Vec<u8>) {
    let mut rng = OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, 1024).unwrap();
    let public_key = RsaPublicKey::from(&private_key);
    let der = public_key.to_public_key_der().unwrap();
    (private_key, der.to_vec())
}

pub fn decrypt_rsa(private_key: &RsaPrivateKey, data: &[u8]) -> Vec<u8> {
    private_key.decrypt(Pkcs1v15Encrypt, data).unwrap()
}

pub fn generate_verify_token() -> Vec<u8> {
    let mut token = vec![0u8; 4];
    OsRng.fill_bytes(&mut token);
    token
}
