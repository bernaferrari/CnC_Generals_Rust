// Auto-generated C++ compatibility shim for RSA
use rand::rngs::OsRng;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};

pub fn generate_key(bits: usize) -> (RsaPrivateKey, RsaPublicKey) {
    let mut rng = OsRng;
    let private = RsaPrivateKey::new(&mut rng, bits).expect("RSA keygen failed");
    let public = RsaPublicKey::from(&private);
    (private, public)
}

pub fn encrypt(public: &RsaPublicKey, data: &[u8]) -> Vec<u8> {
    public
        .encrypt(&mut OsRng, Pkcs1v15Encrypt, data)
        .expect("RSA encrypt failed")
}

pub fn decrypt(private: &RsaPrivateKey, data: &[u8]) -> Vec<u8> {
    private
        .decrypt(Pkcs1v15Encrypt, data)
        .expect("RSA decrypt failed")
}
