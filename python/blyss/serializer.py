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


def get_obj_as_bytes(obj: ClientPayloadType) -> bytes:
    if isinstance(obj, bytes):
        return obj

    obj_json = json.dumps(obj)
    return obj_json.encode()


def get_header_bytes(
    obj: ClientPayloadType, metadata: Optional[dict[Any, Any]] = None
) -> bytes:
    if not metadata and type(obj) == bytes:
        return varint.encode(0)

    header_data = {"contentType": "application/json"}
    if metadata:
        header_data = {**header_data, **metadata}

    header = json.dumps(header_data)
    header_varint = varint.encode(len(header))
    return header_varint + header.encode()


def serialize(obj: Any, metadata: Optional[dict[Any, Any]] = None) -> bytes:
    header_bytes = get_header_bytes(obj, metadata)
    obj_bytes = get_obj_as_bytes(obj)

    return header_bytes + obj_bytes


def deserialize(data: bytes) -> tuple[bytes, Optional[dict[Any, Any]]]:
    header_length = varint.decode_bytes(data)
    bytes_processed = len(varint.encode(header_length))

    i = bytes_processed
    if header_length == 0:
        return (data[i:], None)

    header = json.loads(data[i : i + header_length])
    i += header_length

    obj = data[i:]
    if "contentType" in header and header["contentType"] == "application/json":
        obj = json.loads(obj)

    return (obj, header)


def wrap_key_val(key: bytes, value: bytes) -> bytes:
    """
    Wraps a key and value into a single bytes sequence, following Blyss "kv-item" spec.
    """
    key_len_varint = varint.encode(len(key))
    value_len_varint = varint.encode(len(value))
    return key_len_varint + key + value_len_varint + value
