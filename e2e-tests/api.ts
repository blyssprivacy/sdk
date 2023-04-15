import type { Bucket, Client } from '@blyss/sdk';
const blyss = require('@blyss/sdk/node');

async function keyToValue(key: string, len: number): Promise<Uint8Array> {
    const keyBytes = new TextEncoder().encode(key);
    const value = new Uint8Array(len);
    let i = 0;
    // fill the value with the hash.
    // if the hash is smaller than the value, we hash the hash again.
    while (i < len) {
        const hash = await crypto.subtle.digest('SHA-1', keyBytes);
        const hashBytes = new Uint8Array(hash);
        const toCopy = Math.min(hashBytes.length, len - i);
        value.set(hashBytes.slice(0, toCopy), i);
        i += toCopy;
    }
    return value;
}

async function verifyRead(key: string, value: Uint8Array): Promise<void> {
    const expected = await keyToValue(key, value.length);
    if (expected.toString() !== value.toString()) {
        throw new Error('Incorrect value for key ' + key);
    }
}

function generateKeys(n: number, seed: number = 0): string[] {
    return new Array(n).fill(0).map(
        (_, i) => seed.toString() + '-' + i.toString()
    );
}

function generateBucketName(): string {
    return 'api-tester-' + Math.random().toString(16).substring(2, 10);
}

async function testBlyssService(endpoint: string = 'https://dev2.api.blyss.dev') {
    const apiKey = process.env.BLYSS_API_KEY;
    if (!apiKey) {
        throw new Error('BLYSS_API_KEY environment variable is not set');
    }
    console.log('Using key: ' + apiKey + ' to connect to ' + endpoint);
    const client: Client = await new blyss.Client(
        {
            endpoint: endpoint,
            apiKey: apiKey
        }
    );
    // generate random string for bucket name
    const bucketName = generateBucketName();
    await client.create(bucketName);
    const bucket: Bucket = await client.connect(bucketName);
    console.log(bucket.metadata);

    // generate N random keys
    const N = 100;
    const itemSize = 32;
    let localKeys = generateKeys(N);
    function getRandomKey(): string {
        return localKeys[Math.floor(Math.random() * localKeys.length)];
    }
    // write all N keys
    await bucket.write(
        await Promise.all(localKeys.map(
            async (k) => ({
                k: await keyToValue(k, itemSize)
            })
        ))
    );
    console.log(`Wrote ${N} keys`);

    // read a random key
    let testKey = getRandomKey();
    let value = await bucket.privateRead(testKey);
    await verifyRead(testKey, value);
    console.log(`Read key ${testKey}`);

    // delete testKey from the bucket, and localData.
    await bucket.deleteKey(testKey);
    localKeys.splice(localKeys.indexOf(testKey), 1);
    console.log(`Deleted key ${testKey}`);

    // write a new value
    testKey = 'newKey0';
    await bucket.write({ testKey: keyToValue(testKey, itemSize) });
    localKeys.push(testKey);
    console.log(`Wrote key ${testKey}`);

    // clear all keys
    await bucket.clearEntireBucket();
    localKeys = [];
    console.log('Cleared bucket');

    // write a new set of N keys
    localKeys = generateKeys(N, 1);
    await bucket.write(
        await Promise.all(localKeys.map(
            async (k) => ({
                k: await keyToValue(k, itemSize)
            })
        ))
    );
    console.log(`Wrote ${N} keys`);

    // rename the bucket
    const newBucketName = bucketName + '-rn';
    await bucket.rename(newBucketName);
    console.log(`Renamed bucket`);
    console.log(await bucket.info());

    // random read
    testKey = getRandomKey();
    value = await bucket.privateRead(testKey);
    await verifyRead(testKey, value);
    console.log(`Read key ${testKey}`);

    // destroy the bucket
    await bucket.destroyEntireBucket();
    console.log(`Destroyed bucket ${bucket.name}`);
}

async function main() {
    const endpoint = "https://dev2.api.blyss.dev"
    console.log('Testing Blyss service at URL ' + endpoint);
    await testBlyssService(endpoint);
    console.log('All tests completed successfully.');
}

main();
