"""Seed

INTERNAL

Helper methods to generate and manipulate random seeds.
"""

import os
import base64

SEED_BYTES = 32
SEED_STR_LEN = 44


def _bytes_to_base64(arr: bytes) -> str:
    return base64.standard_b64encode(arr).decode()


def _base64_to_bytes(b64_str: str) -> bytes:
    return base64.standard_b64decode(b64_str)


def string_from_seed(seed: bytes) -> str:
    """Convert bytes to a base64 string seed.

    Args:
        seed (bytes): Raw, random bytes of length SEED_BYTES.

    Returns:
        str: The seed string, of length SEED_STR_LEN.
    """
    assert len(seed) == SEED_BYTES
    seed_str = _bytes_to_base64(seed)
    assert len(seed_str) == SEED_STR_LEN
    return seed_str


def seed_from_string(seed_str: str) -> bytes:
    """Convert a base64 string seed to bytes.

    Args:
        seed_str (str): The seed string, of length SEED_STR_LEN.

    Returns:
        bytes: The raw bytes, of length SEED_BYTES.
    """
    assert len(seed_str) == SEED_STR_LEN
    seed = _base64_to_bytes(seed_str)
    assert len(seed) == SEED_BYTES
    return seed


def get_random_seed() -> str:
    """Generate a random seed using `os.urandom`.

    Returns:
        str: The seed string, of length SEED_STR_LEN.
    """
    seed = os.urandom(SEED_BYTES)
    return string_from_seed(seed)
