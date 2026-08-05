from pathlib import Path
from nacl.signing import SigningKey
from base64 import b64encode

priv = Path("target/release/keys/ed25519_private.key").read_bytes()
sk = SigningKey(priv)
for expiry in [0, 1811497718]:
    payload = f"HHPOS|7181-4532-80F3|{expiry}|PRO"
    sig = b64encode(sk.sign(payload.encode()).signature).decode()
    print("PAYLOAD:", payload)
    print("KEY:", payload + "|" + sig)
    print("SIG_LEN", len(sig))
