import hashlib


def check_bit(data: bytes, i: int) -> bool:
    val = data[i // 8] & (1 << (7 - (i % 8)))
    return val != 0


def top_be_bits(data: bytes, bits: int) -> int:
    num = 0
    for i in range(bits):
        bit = data[i // 8] & (1 << (7 - (i % 8)))
        if bit != 0:
            num += 1 << (bits - 1 - i)
    return num


def to_le_bytes(num: int) -> bytes:
    return num.to_bytes(4, "little")


class BloomFilter:
    def __init__(self, k: int, bits: int, data: bytes):
        self.k = k
        self.bits = bits
        self.data = data

    @staticmethod
    def from_bytes(raw_data: bytes) -> "BloomFilter":
        k = int.from_bytes(raw_data[0:4], "little")
        bits = int.from_bytes(raw_data[4:8], "little")
        data = raw_data[8:]
        return BloomFilter(k, bits, data)

    def hash(self, key: str, hash_idx: int) -> int:
        data_to_hash = to_le_bytes(hash_idx) + key.encode()
        hash_val = hashlib.sha1(data_to_hash).digest()
        num = top_be_bits(hash_val, self.bits)
        return num

    def lookup(self, key: str) -> bool:
        for i in range(self.k):
            idx = self.hash(key, i)
            check = check_bit(self.data, idx)
            if not check:
                return False
        return True
