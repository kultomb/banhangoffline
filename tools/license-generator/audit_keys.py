from pathlib import Path
from base64 import b64decode
from nacl.signing import SigningKey, VerifyKey
import re

root = Path(".")
priv_path = root / "target" / "release" / "keys" / "ed25519_private.key"
pub_path = root / "target" / "release" / "keys" / "ed25519_public.key"
print("priv exists", priv_path.exists())
print("pub exists", pub_path.exists())
priv = priv_path.read_bytes()
pub = pub_path.read_bytes()
print("priv len", len(priv), "pub len", len(pub))

# derive public from private
sk = SigningKey(priv)
derived = sk.verify_key.encode()
print("derived matches pub", derived == pub)
print("pub hex", pub.hex())
print("derived hex", derived.hex())

# parse embedded public key from app-exe/src-tauri/src/crypto.rs
crypto = Path("../../app-exe/src-tauri/src/crypto.rs").read_text()
mask = re.search(r"const CHUNK_MASK: \[u8; 8\] = \[([^\]]+)\];", crypto)
chunks = re.findall(
    r"const PUB_CHUNK_\d: \[u8; 8\] = xor8\(\[([^\]]+)\], CHUNK_MASK\);", crypto
)
if not mask or len(chunks) != 4:
    raise SystemExit("Could not parse crypto.rs")
mask_vals = [int(x.strip(), 0) for x in mask.group(1).split(",")]
embedded = bytearray()
for ch in chunks:
    arr = [int(x.strip(), 0) for x in ch.split(",")]
    embedded.extend([(a ^ mask_vals[i]) for i, a in enumerate(arr)])
print("embedded len", len(embedded), "hex", embedded.hex())
print("embedded matches pub", bytes(embedded) == pub)
print("embedded matches derived", bytes(embedded) == derived)

# verify sample key if present
sample_key = "HHPOS|7181-4532-80F3|1811497718|PRO|WDMDQTRDJoKRjo+IojicCgipFXYSEAQQdAA7xDcYm4427gRs3mhOBX/bAqsHePPsQ1SPu3MaPjr7hyT7GrSDg=="
sample_key = "".join(sample_key.split())
parts = sample_key.split("|")
print("parts", len(parts), parts[:4])
payload = "|".join(parts[:4])
sig = parts[4].replace("-", "")
print("payload", payload)
print("sig len", len(sig))
sig_bytes = b64decode(sig)
print("sig bytes len", len(sig_bytes))
verify_key = VerifyKey(pub)
try:
    verify_key.verify(payload.encode(), sig_bytes)
    print("sample signature valid")
except Exception as e:
    print("sample signature invalid", e)
