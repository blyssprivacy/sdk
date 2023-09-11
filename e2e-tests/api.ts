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


async function generateKVPairs(n: number, seed: number, itemSize: number): Promise<{ [key: string]: Uint8Array }> {
    const keys = generateKeys(n, seed);
    const kvPairs: { [key: string]: Uint8Array } = {};

    for (const key of keys) {
        kvPairs[key] = await keyToValue(key, itemSize);
    }

    return kvPairs;
}


function getRandomKey(kvP: { [key: string]: Uint8Array }): string {
    return Object.keys(kvP)[Math.floor(Math.random() * Object.keys(kvP).length)];
}


function generateBucketName(): string {
    return 'api-tester-' + Math.random().toString(16).substring(2, 10);
}

async function testBlyssService(endpoint: string, apiKey: string) {
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
    let kvPairs = await generateKVPairs(N, 0, itemSize);

    // write all N keys
    await bucket.write(
        kvPairs
    );
    console.log(`Wrote ${N} keys`);

    // read a random key
    let testKey = getRandomKey(kvPairs);
    console.log(`Reading key ${testKey}`)
    await bucket.setup();
    console.log("1111");
    let value = await bucket.privateRead(testKey);
    await verifyRead(testKey, value);
    console.log(`Read key ${testKey}`);

    // delete testKey from the bucket, and localData.
    await bucket.deleteKey(testKey);

    console.log(`Deleted key ${testKey}`);

    // write a new value
    testKey = 'newKey0';
    let newValue = await keyToValue(testKey, itemSize);
    await bucket.write({ testKey: newValue });
    kvPairs[testKey] = newValue;
    console.log(`Wrote key ${testKey}`);

    // clear all keys
    await bucket.clearEntireBucket();
    kvPairs = {};
    console.log('Cleared bucket');

    // write a new set of N keys
    kvPairs = await generateKVPairs(N, 1, itemSize);
    await bucket.write(
        kvPairs
    );
    console.log(`Wrote ${N} keys`);

    // rename the bucket
    const newBucketName = bucketName + '-rn';
    await bucket.rename(newBucketName);
    console.log(`Renamed bucket`);
    console.log(await bucket.info());

    // random read
    testKey = getRandomKey(kvPairs);
    value = await bucket.privateRead(testKey);
    await verifyRead(testKey, value);
    console.log(`Read key ${testKey}`);

    // destroy the bucket
    await bucket.destroyEntireBucket();
    console.log(`Destroyed bucket ${bucket.name}`);
}

async function main(endpoint: string, apiKey: string) {
    if (!apiKey) {
        throw new Error('BLYSS_API_KEY environment variable is not set');
    }
    await testBlyssService(endpoint, apiKey);
    console.log('All tests completed successfully.');
}

// get endpoint and api key from command line, or fallback to defaults
const endpoint = process.argv[2] || 'https://beta.api.blyss.dev';
const apiKey = process.argv[3] || process.env.BLYSS_API_KEY;
main(endpoint, apiKey);
