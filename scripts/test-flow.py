#!/usr/bin/env python3
"""Test the full MPC poker flow: deal → flop → turn → river → showdown."""

import hashlib
import json
import struct
import time
import requests
from nacl.signing import SigningKey

BASE = "http://localhost:8080"
TABLE_ID = 1

# --- Stellar key helpers ---

def encode_stellar_pubkey(raw_32: bytes) -> str:
    """Encode raw ed25519 public key as Stellar G... address."""
    # Version byte 6 << 3 = 48 for ED25519 public key
    payload = bytes([6 << 3]) + raw_32
    # CRC16-XModem checksum
    crc = _crc16_xmodem(payload)
    full = payload + struct.pack("<H", crc)
    return _base32_encode(full)

def _crc16_xmodem(data: bytes) -> int:
    crc = 0
    for byte in data:
        crc ^= byte << 8
        for _ in range(8):
            if crc & 0x8000:
                crc = (crc << 1) ^ 0x1021
            else:
                crc <<= 1
            crc &= 0xFFFF
    return crc

def _base32_encode(data: bytes) -> str:
    import base64
    return base64.b32encode(data).decode("ascii").rstrip("=")

# --- Auth helpers ---

def make_auth_headers(signing_key: SigningKey, address: str, table_id: int, action: str, nonce: int) -> dict:
    timestamp = int(time.time())
    message = f"stellar-poker|{address}|{table_id}|{action}|{nonce}|{timestamp}"
    sig = signing_key.sign(message.encode()).signature
    return {
        "x-player-address": address,
        "x-auth-signature": sig.hex(),
        "x-auth-nonce": str(nonce),
        "x-auth-timestamp": str(timestamp),
        "Content-Type": "application/json",
    }

# --- Generate two player keypairs ---

sk1 = SigningKey.generate()
sk2 = SigningKey.generate()
addr1 = encode_stellar_pubkey(bytes(sk1.verify_key))
addr2 = encode_stellar_pubkey(bytes(sk2.verify_key))

print(f"Player 1: {addr1}")
print(f"Player 2: {addr2}")

nonce = {addr1: 0, addr2: 0}

def next_nonce(addr):
    nonce[addr] += 1
    return nonce[addr]

# --- Step 1: Health check ---
print("\n=== Health Check ===")
r = requests.get(f"{BASE}/api/health")
print(f"  {r.status_code}: {r.text}")

print("\n=== Committee Status ===")
r = requests.get(f"{BASE}/api/committee/status")
print(f"  {r.status_code}: {r.json()}")

# --- Step 2: Request Deal ---
print("\n=== Request Deal (table 1, 2 players) ===")
headers = make_auth_headers(sk1, addr1, TABLE_ID, "request_deal", next_nonce(addr1))
payload = {"players": [addr1, addr2]}
r = requests.post(f"{BASE}/api/table/{TABLE_ID}/request-deal", json=payload, headers=headers, timeout=120)
print(f"  Status: {r.status_code}")
print(f"  Headers: {dict(r.headers)}")
print(f"  Body: {r.text[:2000]}")
if r.status_code == 200:
    deal = r.json()
    print(f"  Deal response: {json.dumps(deal, indent=2)}")
else:
    print(f"  Deal failed — stopping here.")
    exit(1)

# --- Step 3: Request Reveal Flop ---
print("\n=== Request Reveal: Flop ===")
headers = make_auth_headers(sk1, addr1, TABLE_ID, "request_reveal:flop", next_nonce(addr1))
r = requests.post(f"{BASE}/api/table/{TABLE_ID}/request-reveal/flop", headers=headers, timeout=120)
print(f"  Status: {r.status_code}")
if r.status_code == 200:
    flop = r.json()
    print(f"  Flop cards: {flop['cards']}")
    print(f"  Proof size: {flop['proof_size']}")
else:
    print(f"  Error: {r.text}")
    exit(1)

# --- Step 4: Request Reveal Turn ---
print("\n=== Request Reveal: Turn ===")
headers = make_auth_headers(sk1, addr1, TABLE_ID, "request_reveal:turn", next_nonce(addr1))
r = requests.post(f"{BASE}/api/table/{TABLE_ID}/request-reveal/turn", headers=headers, timeout=120)
print(f"  Status: {r.status_code}")
if r.status_code == 200:
    turn = r.json()
    print(f"  Turn card: {turn['cards']}")
    print(f"  Proof size: {turn['proof_size']}")
else:
    print(f"  Error: {r.text}")
    exit(1)

# --- Step 5: Request Reveal River ---
print("\n=== Request Reveal: River ===")
headers = make_auth_headers(sk1, addr1, TABLE_ID, "request_reveal:river", next_nonce(addr1))
r = requests.post(f"{BASE}/api/table/{TABLE_ID}/request-reveal/river", headers=headers, timeout=120)
print(f"  Status: {r.status_code}")
if r.status_code == 200:
    river = r.json()
    print(f"  River card: {river['cards']}")
    print(f"  Proof size: {river['proof_size']}")
else:
    print(f"  Error: {r.text}")
    exit(1)

# --- Step 6: Request Showdown ---
print("\n=== Request Showdown ===")
headers = make_auth_headers(sk1, addr1, TABLE_ID, "request_showdown", next_nonce(addr1))
r = requests.post(f"{BASE}/api/table/{TABLE_ID}/request-showdown", headers=headers, timeout=120)
print(f"  Status: {r.status_code}")
if r.status_code == 200:
    showdown = r.json()
    print(f"  Winner: {showdown['winner']}")
    print(f"  Winner index: {showdown['winner_index']}")
    print(f"  Proof size: {showdown['proof_size']}")
else:
    print(f"  Error: {r.text}")
    exit(1)

print("\n=== FULL FLOW COMPLETE ===")
