from typing import Optional

import os
import sys
import random
import hashlib
import traceback
import blyss


def key_to_gold_value(key: str, length: int = 512) -> bytes:
    h = hashlib.md5()
    h.update(key.encode("utf-8"))
    value = h.digest()
    while len(value) < length:
        h.update(b"0")
        value += h.digest()
    return value[:length]


def verify_read(key: str, value: bytes):
    expected = key_to_gold_value(key, len(value))
    try:
        assert value == expected
    except:
        print(f"read mismatch for key {key}")
        print(f"received {value.hex()[:16]}")
        print(f"expected {expected.hex()[:16]}")

        print(traceback.format_exc())
        raise


def generate_keys(n: int, seed: int = 0) -> list:
    return [f"{seed}-{i}" for i in range(n)]


def generateBucketName() -> str:
    tag = int(random.random() * 1e6)
    return f"api-tester-{tag:#0{6}x}"


async def test_e2e_async(
    endpoint: str, api_key: str, N: int = 4000, itemSize: int = 32
):
    client = blyss.AsyncClient(api_key, endpoint)
    # generate random string for bucket name
    bucket_name = generateBucketName()
    await client.create(bucket_name, usage_hints={"maxItemSize": 40_000})
    print("Created bucket")
    bucket = await client.connect(bucket_name)
    print(await bucket.info())

    # generate N random keys
    local_keys = generate_keys(N, 0)
    # write all N keys
    await bucket.write({k: key_to_gold_value(k, itemSize) for k in local_keys})
    print(f"Wrote {N} keys")

    # read a random key
    testKey = random.choice(local_keys)
    value = (await bucket.private_read([testKey]))[0]
    assert value is not None
    verify_read(testKey, value)
    print(f"Read key {testKey}, got {value.hex()[:8]}[...]")

    # delete testKey from the bucket, and localData.
    await bucket.delete_key(testKey)
    local_keys.remove(testKey)
    value = (await bucket.private_read([testKey]))[0]

    def _test_delete(key: str, value: Optional[bytes]):
        if value is None:
            print(f"Deleted key {key}")
        else:
            # this happens only sometimes??
            print("ERROR: delete not reflected in read!")
            print(f"Read deleted key {key} and got {value.hex()[:8]}[...]")

    _test_delete(testKey, value)

    # clear all keys
    await bucket.clear_entire_bucket()
    local_keys = []
    print("Cleared bucket")

    # write a new set of N keys
    local_keys = generate_keys(N, 2)
    await bucket.write({k: key_to_gold_value(k, itemSize) for k in local_keys})
    print(f"Wrote {N} keys")

    # read a random key
    testKey = random.choice(local_keys)
    value = (await bucket.private_read([testKey]))[0]
    assert value is not None
    verify_read(testKey, value)

    # rename the bucket
    new_bucket_name = bucket_name + "-rn"
    await bucket.rename(new_bucket_name)
    print("Renamed bucket")
    print(await bucket.info())

    # read a random key
    testKey = random.choice(local_keys)
    value = (await bucket.private_read([testKey]))[0]
    assert value is not None
    verify_read(testKey, value)
    print(f"Read key {testKey}")

    # destroy the bucket
    await bucket.destroy_entire_bucket()
    print("Destroyed bucket")


def test_e2e(endpoint: str, api_key: str, N: int = 4000, itemSize: int = 32):
    client = blyss.Client(api_key, endpoint)
    # generate random string for bucket name
    bucket_name = generateBucketName()
    client.create(bucket_name, usage_hints={"maxItemSize": 40_000})
    print("Created bucket")
    bucket = client.connect(bucket_name)
    print(bucket.info())

    # generate N random keys
    local_keys = generate_keys(N, 0)
    # write all N keys
    bucket.write({k: key_to_gold_value(k, itemSize) for k in local_keys})
    print(f"Wrote {N} keys")

    # read a random key
    testKey = random.choice(local_keys)
    value = bucket.private_read([testKey])[0]
    assert value is not None
    verify_read(testKey, value)
    print(f"Read key {testKey}, got {value.hex()[:8]}[...]")

    # delete testKey from the bucket, and localData.
    bucket.delete_key(testKey)
    local_keys.remove(testKey)
    value = bucket.private_read([testKey])[0]

    def _test_delete(key: str, value: Optional[bytes]):
        if value is None:
            print(f"Deleted key {key}")
        else:
            # this happens only sometimes??
            print("ERROR: delete not reflected in read!")
            print(f"Read deleted key {key} and got {value.hex()[:8]}[...]")

    _test_delete(testKey, value)

    # clear all keys
    bucket.clear_entire_bucket()
    local_keys = []
    print("Cleared bucket")

    # write a new set of N keys
    local_keys = generate_keys(N, 2)
    bucket.write({k: key_to_gold_value(k, itemSize) for k in local_keys})
    print(f"Wrote {N} keys")

    # read a random key
    testKey = random.choice(local_keys)
    value = bucket.private_read([testKey])[0]
    assert value is not None
    verify_read(testKey, value)

    # rename the bucket
    new_bucket_name = bucket_name + "-rn"
    bucket.rename(new_bucket_name)
    print("Renamed bucket")
    print(bucket.info())

    # read a random key
    testKey = random.choice(local_keys)
    value = bucket.private_read([testKey])[0]
    assert value is not None
    verify_read(testKey, value)
    print(f"Read key {testKey}")

    # destroy the bucket
    bucket.destroy_entire_bucket()
    print("Destroyed bucket")


if __name__ == "__main__":
    import asyncio

    api_key = os.environ.get("BLYSS_STAGING_API_KEY", None)
    endpoint = os.environ.get("BLYSS_STAGING_SERVER", None)
    if len(sys.argv) > 1:
        print("Using endpoint from command line")
        endpoint = sys.argv[1]
    if len(sys.argv) > 2:
        print("Using api_key from command line")
        api_key = sys.argv[2]
        if api_key == "none":
            api_key = None
    print("DEBUG", api_key, endpoint)
    assert endpoint is not None
    assert api_key is not None

    print(f"testing Blyss endpoint at {endpoint}")
    asyncio.run(test_e2e_async(endpoint, api_key))
    print("async ✅")

    test_e2e(endpoint, api_key)
    print("sync ✅")
