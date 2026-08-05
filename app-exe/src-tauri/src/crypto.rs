use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

// ---------------------------------------------------------------------------
// Ed25519 public key — embedded in binary (split into 4 chunks + XOR mask to
// make naive binary patching harder).
//
// Public key updated from generated admin keypair.
// This value corresponds to tools/license-generator/target/release/keys/ed25519_public.key.
// Keep chunks masked via XOR to avoid plain byte sequence in binary.
// ---------------------------------------------------------------------------

const CHUNK_MASK: [u8; 8] = [0xA3, 0x7F, 0x12, 0xE8, 0x5C, 0x94, 0x2B, 0x66];

const PUB_CHUNK_0: [u8; 8] = xor8([0xF5, 0x47, 0x27, 0xB4, 0xAB, 0xFE, 0x3D, 0x63], CHUNK_MASK);
const PUB_CHUNK_1: [u8; 8] = xor8([0xEB, 0x88, 0x5F, 0xDB, 0x34, 0x59, 0x07, 0x0C], CHUNK_MASK);
const PUB_CHUNK_2: [u8; 8] = xor8([0x62, 0x98, 0x99, 0x37, 0x2C, 0x53, 0x60, 0xE7], CHUNK_MASK);
const PUB_CHUNK_3: [u8; 8] = xor8([0x0B, 0x8B, 0x93, 0x85, 0xE5, 0x69, 0x4A, 0xF8], CHUNK_MASK);

const fn xor8(a: [u8; 8], b: [u8; 8]) -> [u8; 8] {
    [
        a[0] ^ b[0],
        a[1] ^ b[1],
        a[2] ^ b[2],
        a[3] ^ b[3],
        a[4] ^ b[4],
        a[5] ^ b[5],
        a[6] ^ b[6],
        a[7] ^ b[7],
    ]
}

/// Reconstruct the 32-byte Ed25519 public key from the four masked chunks.
fn try_load_external_pub_key() -> Option<[u8; 32]> {
    fn read_key(path: &Path) -> Option<[u8; 32]> {
        let data = fs::read(path).ok()?;
        if data.len() == 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(&data);
            return Some(out);
        }
        let text = String::from_utf8(data).ok()?;
        let decoded = STANDARD.decode(text.trim()).ok()?;
        if decoded.len() == 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(&decoded);
            return Some(out);
        }
        None
    }

    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))?;
    let candidates = [
        exe_dir.join("ed25519_public.key"),
        exe_dir.join("public_key.bin"),
        exe_dir.join("public_key.txt"),
    ];

    for path in candidates.iter() {
        if let Some(key) = read_key(path) {
            return Some(key);
        }
    }
    None
}

fn get_pub_key_bytes() -> [u8; 32] {
    if let Some(external) = try_load_external_pub_key() {
        return external;
    }

    let mut out = [0u8; 32];
    for (i, &b) in PUB_CHUNK_0.iter().enumerate() {
        out[i] = b ^ CHUNK_MASK[i];
    }
    for (i, &b) in PUB_CHUNK_1.iter().enumerate() {
        out[8 + i] = b ^ CHUNK_MASK[i];
    }
    for (i, &b) in PUB_CHUNK_2.iter().enumerate() {
        out[16 + i] = b ^ CHUNK_MASK[i];
    }
    for (i, &b) in PUB_CHUNK_3.iter().enumerate() {
        out[24 + i] = b ^ CHUNK_MASK[i];
    }
    out
}

// ---------------------------------------------------------------------------
// Signature verification
// ---------------------------------------------------------------------------

/// Verify an Ed25519 signature over `payload` (UTF-8).
/// `sig_b64` is the standard-Base64 encoding of the 64-byte signature.
///
/// Returns `true` only if the key is valid AND the signature checks out.
pub fn verify_signature(payload: &str, sig_b64: &str) -> bool {
    let key_bytes = get_pub_key_bytes();
    let verifying_key = match VerifyingKey::from_bytes(&key_bytes) {
        Ok(k) => k,
        Err(_) => return false,
    };

    let sig_bytes = match STANDARD.decode(sig_b64) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let sig_arr: [u8; 64] = match sig_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(&sig_arr);

    verifying_key
        .verify(payload.as_bytes(), &signature)
        .is_ok()
}

// ---------------------------------------------------------------------------
// XOR-stream cipher for license.dat / trial.dat / runtime.enc
//
// The encryption key is SHA-256(machine_id || SALT), so the file is
// unreadable on any machine with a different machine_id.
// ---------------------------------------------------------------------------

const FILE_SALT: &[u8] = b"HangHoaPOS_licdat_salt_v1_2025!";

/// Derive a 32-byte file-encryption key from the machine ID.
pub fn derive_file_key(machine_id: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(machine_id.as_bytes());
    h.update(FILE_SALT);
    h.finalize().into()
}

/// XOR-stream cipher (encrypt == decrypt — same function).
pub fn xor_crypt(data: &[u8], key: &[u8; 32]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % 32])
        .collect()
}
