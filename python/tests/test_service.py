import os
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
    try:
        assert value == key_to_gold_value(key, len(value))
    except:
        print(traceback.format_exc())
        raise


def generate_keys(n: int, seed: int = 0) -> list:
    return [f"{seed}-{i}" for i in range(n)]


def generateBucketName() -> str:
    tag = int(random.random() * 1e6)
    return f"api-tester-{tag:#0{6}x}"


async def main(endpoint: str):
    api_key = os.environ.get("BLYSS_API_KEY", None)
    if not api_key:
        raise Exception("BLYSS_API_KEY environment variable is not set")
    print("Using key: " + api_key + " to connect to " + endpoint)
    client = blyss.Client({"endpoint": endpoint, "api_key": api_key})
    # generate random string for bucket name
    bucketName = generateBucketName()
    client.create(bucketName)
    bucket = client.connect_async(bucketName)
    print(bucket.info())

    # generate N random keys
    N = 20000
    itemSize = 32
    localKeys = generate_keys(N, 0)
    # write all N keys
    await bucket.write({k: key_to_gold_value(k, itemSize) for k in localKeys})
    print(f"Wrote {N} keys")

    # read a random key
    testKey = random.choice(localKeys)
    value = await bucket.private_read([testKey])
    verify_read(testKey, value[0])
    print(f"Read key {testKey}")

    # delete testKey from the bucket, and localData.
    bucket.delete_key(testKey)
    localKeys.remove(testKey)
    value = await bucket.private_read([testKey])
    # TODO: why aren't deletes reflected in the next read?
    # assert value is None
    print(f"Deleted key {testKey}")

    # clear all keys
    bucket.clear_entire_bucket()
    localKeys = []
    print("Cleared bucket")

    # write a new set of N keys
    localKeys = generate_keys(N, 2)
    await bucket.write({k: key_to_gold_value(k, itemSize) for k in localKeys})
    print(f"Wrote {N} keys")

    # test if clear took AFTER the new write
    value = await bucket.private_read([testKey])
    if value is not None:
        print(f"ERROR: {testKey} was not deleted or cleared!")

    # rename the bucket
    newBucketName = bucketName + "-rn"
    bucket.rename(newBucketName)
    print("Renamed bucket")
    print(bucket.info())

    # read a random key
    testKey = random.choice(localKeys)
    value = await bucket.private_read([testKey])
    verify_read(testKey, value[0])
    print(f"Read key {testKey}")

    # destroy the bucket
    bucket.destroy_entire_bucket()
    print("Destroyed bucket")


if __name__ == "__main__":
    import asyncio

    asyncio.run(main("https://dev2.api.blyss.dev"))
