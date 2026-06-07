use rand::Rng;

pub fn generate_secret() -> String {
    let mut bytes = [0_u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
