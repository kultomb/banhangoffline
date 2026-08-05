use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::fs;

fn main() {
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    let verifying_key = signing_key.verifying_key();
    
    let priv_bytes = signing_key.to_bytes();
    let pub_bytes = verifying_key.to_bytes();
    
    // Create keys dir if not exists
    let _ = fs::create_dir("keys");
    
    fs::write("keys/ed25519_private.key", priv_bytes)
        .expect("Failed to write private key");
    fs::write("keys/ed25519_public.key", pub_bytes)
        .expect("Failed to write public key");
    
    println!("Private key (hex): {}", hex::encode(priv_bytes));
    println!("Public key (hex):  {}", hex::encode(pub_bytes));
    
    let pub_bytes_fmt = pub_bytes
        .iter()
        .map(|b| format!("0x{:02x}", b))
        .collect::<Vec<_>>()
        .join(", ");
    
    println!("\nPublic key for crypto.rs:");
    println!("[{}]", pub_bytes_fmt);
}
