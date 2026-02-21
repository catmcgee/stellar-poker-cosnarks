#!/usr/bin/env python3
"""Convert a Barretenberg VK binary (3680 bytes, limb-encoded) to formats needed
by the Soroban UltraHonk verifier and co-noir keccak prover.

BB format (3680 bytes) — "poseidon2" / limb-encoded:
  3 × 32-byte big-endian headers: log_circuit_size, num_public_inputs, pub_inputs_offset
  28 × 128-byte G1 points: each as (x_lo, x_hi, y_lo, y_hi) with 32 bytes per limb

Soroban compact format (1760 bytes):
  4 × 8-byte big-endian u64 headers: circuit_size, log_circuit_size, public_inputs_size, pub_inputs_offset
  27 × 64-byte G1 points: each as (x, y) with 32 bytes per coordinate

co-noir keccak format (1888 bytes):
  3 × 32-byte big-endian headers: log_circuit_size, num_public_inputs, pub_inputs_offset
  28 × 64-byte G1 points: each as (x, y) with 32 bytes per coordinate

Usage:
  python3 convert-vk.py <input_vk> <output_soroban> [<output_keccak>]
"""

import sys
import struct


def combine_limbs(lo: bytes, hi: bytes) -> bytes:
    """Reconstruct a 32-byte big-endian coordinate from (lo136, hi) limb pair."""
    out = bytearray(32)
    out[0:15] = hi[17:32]   # upper 15 bytes from hi
    out[15:32] = lo[15:32]  # lower 17 bytes from lo
    return bytes(out)


def parse_bb_vk(data: bytes):
    """Parse a BB VK binary into headers and G1 points."""
    if len(data) != 3680:
        raise ValueError(f"Unexpected VK size: {len(data)} bytes (expected 3680)")

    log_circuit_size = int.from_bytes(data[0:32], "big")
    num_public_inputs = int.from_bytes(data[32:64], "big")
    pub_inputs_offset = int.from_bytes(data[64:96], "big")
    circuit_size = 1 << log_circuit_size

    points = []
    offset = 96
    for i in range(28):
        x_lo = data[offset:offset + 32]
        x_hi = data[offset + 32:offset + 64]
        y_lo = data[offset + 64:offset + 96]
        y_hi = data[offset + 96:offset + 128]
        x = combine_limbs(x_lo, x_hi)
        y = combine_limbs(y_lo, y_hi)
        points.append((x, y))
        offset += 128

    return log_circuit_size, num_public_inputs, pub_inputs_offset, circuit_size, points


def write_soroban_compact(output_path, log_circuit_size, num_public_inputs, pub_inputs_offset, circuit_size, points):
    """Write Soroban compact VK (1824 bytes): 4×u64 header + 28 G1 points."""
    out = bytearray()
    out += struct.pack(">Q", circuit_size)
    out += struct.pack(">Q", log_circuit_size)
    out += struct.pack(">Q", num_public_inputs)
    out += struct.pack(">Q", pub_inputs_offset)

    for i in range(28):  # All 28 precomputed entity commitments
        x, y = points[i]
        out += x
        out += y

    assert len(out) == 1824, f"Soroban output size mismatch: {len(out)} != 1824"
    open(output_path, "wb").write(out)
    return len(out)


def write_keccak_vk(output_path, log_circuit_size, num_public_inputs, pub_inputs_offset, points):
    """Write co-noir keccak VK (1888 bytes): 3×32-byte header + 28 G1 points."""
    out = bytearray()
    out += log_circuit_size.to_bytes(32, "big")
    out += num_public_inputs.to_bytes(32, "big")
    out += pub_inputs_offset.to_bytes(32, "big")

    for i in range(28):  # All 28 points
        x, y = points[i]
        out += x
        out += y

    assert len(out) == 1888, f"Keccak output size mismatch: {len(out)} != 1888"
    open(output_path, "wb").write(out)
    return len(out)


def convert_vk(input_path: str, output_soroban: str, output_keccak: str = None):
    data = open(input_path, "rb").read()

    if len(data) == 1760:
        print(f"  VK already in Soroban compact format ({len(data)} bytes), copying as-is.")
        open(output_soroban, "wb").write(data)
        return

    log_cs, num_pi, pi_off, cs, points = parse_bb_vk(data)
    print(f"  log_circuit_size={log_cs}, circuit_size={cs}")
    print(f"  num_public_inputs={num_pi}, pub_inputs_offset={pi_off}")

    sz = write_soroban_compact(output_soroban, log_cs, num_pi, pi_off, cs, points)
    print(f"  Soroban compact: {len(data)} -> {sz} bytes ({output_soroban})")

    if output_keccak:
        sz = write_keccak_vk(output_keccak, log_cs, num_pi, pi_off, points)
        print(f"  co-noir keccak:  {len(data)} -> {sz} bytes ({output_keccak})")


if __name__ == "__main__":
    if len(sys.argv) < 3 or len(sys.argv) > 4:
        print(f"Usage: {sys.argv[0]} <input_vk> <output_soroban> [<output_keccak>]")
        sys.exit(1)
    output_keccak = sys.argv[3] if len(sys.argv) == 4 else None
    convert_vk(sys.argv[1], sys.argv[2], output_keccak)
