"""Serialization

INTERNAL

Methods to serialize and deserialize data for storage in Blyss.
"""

from typing import Optional, Union, Any
import json
from . import varint

# Set of acceptable object types for client payload.
# Essentially, any JSONable type or raw bytes.
ClientPayloadType = Union[bytes, str, list[Any], dict[Any, Any]]


def wrap_key_val(key: bytes, value: bytes) -> bytes:
    """
    Wraps a key and value into a single bytes sequence, following Blyss "kv-item" spec.
    """
    key_len_varint = varint.encode(len(key))
    value_len_varint = varint.encode(len(value))
    return key_len_varint + key + value_len_varint + value
