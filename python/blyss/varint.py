"""Varint encoder/decoder

varints are a common encoding for variable length integer data, used in
libraries such as sqlite, protobuf, v8, and more.
"""

from io import BytesIO
from typing import BinaryIO


def _byte(b: int) -> bytes:
    return bytes((b,))


def encode(number: int) -> bytes:
    """Encode the given number as a varint.

    Args:
        number (int): An integer to encode.

    Returns:
        bytes: An encoding of the supplied integer as a varint.
    """
    buf = b""
    while True:
        towrite = number & 0x7F
        number >>= 7
        if number:
            buf += _byte(towrite | 0x80)
        else:
            buf += _byte(towrite)
            break
    return buf


def decode_stream(stream: BinaryIO) -> int:
    """Decode a varint from a stream.

    Args:
        stream (BinaryIO): A read()-able object to read a varint from.

    Returns:
        int: The value read from the stream.
    """
    shift = 0
    result = 0
    while True:
        i = _read_one(stream)
        result |= (i & 0x7F) << shift
        shift += 7
        if not (i & 0x80):
            break

    return result


def decode_bytes(buf: bytes):
    """Read a varint from from `buf` bytes"""
    return decode_stream(BytesIO(buf))


def _read_one(stream: BinaryIO):
    """Read a byte from the file (as an integer)
    raises EOFError if the stream ends while reading bytes.
    """
    c = stream.read(1)
    if c == b"":
        raise EOFError("Unexpected EOF while reading bytes")
    return ord(c)
