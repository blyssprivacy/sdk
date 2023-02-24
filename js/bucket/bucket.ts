import { Api, ApiError, BucketMetadata } from '../client/api';
import { base64ToBytes, getRandomSeed } from '../client/seed';
import { decompress } from '../compression/bz2_decompress';
import { bloomLookup } from '../data/bloom';
import {
  DataWithMetadata,
  concatBytes,
  deserialize,
  deserializeChunks,
  serialize,
  serializeChunks,
  wrapKeyValue
} from '../data/serializer';
import { BlyssLib, DoublePIRApiClient } from '../lib/blyss_lib';

/**
 * Maximum number of private reads to perform before using the Bloom filter
 * optimization.
 */
export const BLOOM_CUTOFF = 0;

/** Information about a key in a bucket. */
export interface KeyInfo {
  /** The name of the key, as a string. */
  name?: string;
}

/**
 * A client to a single Blyss bucket.
 *
 * You should not need to construct this object directly. Instead, call
 * `client.connect()` to connect to an existing bucket, or `client.create()` to
 * create a new one.
 *
 * You can serialize and deserialize this object using `toSecretSeed()` and
 * `client.connect(bucketName, secretSeed)`.
 */
export class Bucket {
  /** The target API to send all underlying API calls to. */
  readonly api: Api;

  /** The name of this bucket. */
  readonly name: string;

  /**
   * The secret seed for this instance of the client, which can be saved and
   * then later used to restore state.
   */
  readonly secretSeed?: string;

  /** The metadata of this bucket. */
  metadata: BucketMetadata;

  /** The underlying PIR scheme to use to access this bucket. */
  scheme: 'spiral' | 'doublepir';

  /** The inner WASM client for this instance of the client. */
  lib: BlyssLib;

  /** The inner DoublePIR WASM client for this instance of the client. */
  dpLib: DoublePIRApiClient;

  /** The public UUID of this client's public parameters. */
  uuid?: string;

  /** The cached hint downloaded from a checklist-enabled bucket. */
  hint?: Uint8Array;

  /**
   * The maximum size of query batches sent to the service. Must be greater than
   * 0.
   */
  batchSize = 5;

  private constructor(api: Api, name: string, secretSeed?: string) {
    this.api = api;
    this.name = name;
    this.secretSeed = getRandomSeed();
    this.scheme = 'spiral';
    if (secretSeed) {
      this.secretSeed = secretSeed;
    }
  }

  private async check(uuid: string): Promise<boolean> {
    try {
      await this.api.check(uuid);
      return true;
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) {
        return false;
      } else {
        throw e;
      }
    }
  }

  private async getEndResult(key: string, queryResult: Uint8Array) {
    const decryptedResult = this.lib.decodeResponse(queryResult);

    let decompressedResult = null;
    try {
      decompressedResult = decompress(decryptedResult);
    } catch (e) {
      console.error('decompress error', e);
    }
    if (decompressedResult === null) {
      return null;
    }

    let extractedResult = null;
    try {
      extractedResult = this.lib.extractResult(key, decompressedResult);
    } catch (e) {
      console.error('extraction error', e);
    }
    if (extractedResult === null) {
      return null;
    }

    const result = deserialize(extractedResult);
    return result;
  }

  private async getRawResponse(queryData: Uint8Array): Promise<Uint8Array> {
    const queryResult = base64ToBytes(
      new TextDecoder().decode(await this.api.privateRead(this.name, queryData))
    );
    return queryResult;
  }

  private async getRawResponseMultipart(
    queryData: Uint8Array
  ): Promise<Uint8Array> {
    const targetUrl = this.metadata['pir_scheme']['url'];
    const queryResult = base64ToBytes(
      new TextDecoder().decode(
        await this.api.privateReadMultipart(this.name, queryData, targetUrl)
      )
    );
    return queryResult;
  }

  private async performPrivateReads(
    keys: string[]
  ): Promise<DataWithMetadata[]> {
    if (!this.uuid || !this.check(this.uuid)) {
      await this.setup();
    }

    const queries: { key: string; queryData: Uint8Array }[] = [];
    for (const key of keys) {
      const rowIdx = this.lib.getRow(key);
      const queryData = this.lib.generateQuery(this.uuid, rowIdx);
      queries.push({ key, queryData });
    }

    const endResults = [];
    const batches = Math.ceil(queries.length / this.batchSize);
    for (let i = 0; i < batches; i++) {
      const queriesForBatch = queries.slice(
        i * this.batchSize,
        (i + 1) * this.batchSize
      );

      const queryBatch = serializeChunks(queriesForBatch.map(x => x.queryData));
      const rawResultChunks = await this.getRawResponse(queryBatch);
      const rawResults = deserializeChunks(rawResultChunks);

      const batchEndResults = await Promise.all(
        rawResults.map((r, i) => this.getEndResult(queriesForBatch[i].key, r))
      );

      endResults.push(...batchEndResults);
    }

    return endResults;
  }

  private async performPrivateRead(key: string): Promise<DataWithMetadata> {
    return (await this.performPrivateReads([key]))[0];
  }

  private async getHint(): Promise<Uint8Array> {
    if (!this.hint) {
      this.hint = await this.api.hint(this.name);
    }
    return this.hint;
  }

  private async performDoublePIRRead(key: string): Promise<boolean> {
    const idx = this.dpLib.get_row(key);
    const hintPromise = this.getHint();
    const query = this.dpLib.generate_query(idx);
    const queryResult = this.getRawResponse(query);
    this.dpLib.load_hint(await hintPromise);
    const decryptedResult = this.dpLib.decode_response(await queryResult);
    const extractedResult = this.dpLib.extract_result(decryptedResult);
    return extractedResult;
  }

  private async performDoublePIRReadBatch(key: string): Promise<boolean> {
    const indices = this.dpLib.get_bloom_indices(key, 8, 36);

    const hint = this.getHint();
    const query = await this.dpLib.generate_query_batch_fast(indices);
    const queryResult = this.getRawResponseMultipart(query);
    this.dpLib.load_hint(await hint);
    const result = await queryResult;

    const decryptedResult = this.dpLib.decode_response_batch(result);

    let count = 0;
    for (let i = 0; i < decryptedResult.length; i++) {
      const bit = decryptedResult[i];
      if (bit == 1) {
        count += 1;
      } else if (bit == 0) {
        return false;
      }
    }

    return count >= 5;
  }

  private ensureSpiral() {
    if (this.scheme !== 'spiral')
      throw 'Cannot perform this action on this bucket';
  }

  private ensureDoublePIR() {
    if (this.scheme !== 'doublepir')
      throw 'Cannot perform this action on this bucket';
  }

  /**
   * Initialize a client for a single existing Blyss bucket. You should not need
   * to call this method directly. Instead, call `client.connect()` to connect
   * to an existing bucket, or `client.create()` to create a new one.
   *
   * @param {Api} api - A target API to send all underlying API calls to.
   * @param {string} name - The name of the bucket.
   * @param {string} [secretSeed] - An optional secret seed to initialize the
   *   client with. A random one will be generated if not supplied.
   */
  static async initialize(
    api: Api,
    name: string,
    secretSeed?: string
  ): Promise<Bucket> {
    const b = new this(api, name, secretSeed);
    b.metadata = await b.api.meta(b.name);
    const scheme = b.metadata.pir_scheme;
    if (scheme['scheme'] && scheme['scheme'] === 'doublepir') {
      scheme['num_entries'] = '' + scheme['num_entries'];
      b.scheme = 'doublepir';

      b.dpLib = await DoublePIRApiClient.initialize_client(
        JSON.stringify(scheme)
      );
    } else {
      b.scheme = 'spiral';
      b.lib = new BlyssLib(JSON.stringify(scheme), b.secretSeed);
    }
    return b;
  }

  /**
   * Prepares this bucket client for private reads.
   *
   * This method will be called automatically by
   * {@link privateRead(key: string)}, but clients may call it explicitly prior
   * to make subsequent {@link privateRead(key: string)} calls faster.
   *
   * Can upload significant amounts of data (1-10 MB).
   *
   * @param {string} [uuid] - Optional previous UUID that the client should
   *   attempt to reuse, to avoid generating and uploading larger amounts of
   *   data.
   */
  async setup(uuid?: string) {
    this.ensureSpiral();
    if (uuid && this.check(uuid)) {
      this.lib.generateKeys(false);
      this.uuid = uuid;
    } else {
      const publicParams = this.lib.generateKeys(true);
      const setupResp = await this.api.setup(this.name, publicParams);
      this.uuid = setupResp.uuid;
    }
  }

  /** Gets information about this bucket from the service. */
  async info(): Promise<BucketMetadata> {
    return await this.api.meta(this.name);
  }

  /** Gets info on all keys in this bucket. */
  async listKeys(): Promise<KeyInfo[]> {
    this.ensureSpiral();
    return await this.api.listKeys(this.name);
  }

  /**
   * Make a write to this bucket.
   *
   * @param {{ [key: string]: any }} keyValuePairs - An object containing the
   *   key-value pairs to write. Keys must be strings, and values may be any
   *   JSON-serializable value or a Uint8Array. The maximum size of a key is
   *   1024 UTF-8 bytes.
   * @param {{ [key: string]: any }} [metadata] - An optional object containing
   *   metadata. Each key of this object should also be a key of
   *   `keyValuePairs`, and the value should be some metadata object to store
   *   with the values being written.
   */
  async write(
    keyValuePairs: { [key: string]: any },
    metadata?: { [key: string]: any }
  ) {
    this.ensureSpiral();

    const data = [];
    for (const key in keyValuePairs) {
      if (Object.prototype.hasOwnProperty.call(keyValuePairs, key)) {
        const value = keyValuePairs[key];
        let valueMetadata = undefined;
        if (metadata && Object.prototype.hasOwnProperty.call(metadata, key)) {
          valueMetadata = metadata[key];
        }
        const valueBytes = serialize(value, valueMetadata);
        const keyBytes = new TextEncoder().encode(key);
        const serializedKeyValue = wrapKeyValue(keyBytes, valueBytes);
        data.push(serializedKeyValue);
      }
    }
    const concatenatedData = concatBytes(data);
    await this.api.write(this.name, concatenatedData);
  }

  /**
   * Deletes the supplied key from the bucket.
   *
   * Note that this does not remove the key from the Bloom filter, so subsequent
   * calls to `privateIntersect` or `privateKeyIntersect` could still return
   * this key.
   *
   * @param {string} key - The key to delete.
   */
  async deleteKey(key: string) {
    this.ensureSpiral();

    await this.api.deleteKey(this.name, key);
  }

  /**
   * Destroys the entire bucket, and all data inside of it. This action is
   * permanent and irreversible.
   */
  async destroyEntireBucket() {
    await this.api.destroy(this.name);
  }

  /**
   * Privately reads the supplied key from the bucket, returning the value
   * corresponding to the key.
   *
   * No entity, including the Blyss service, should be able to determine which
   * key this method was called for.
   *
   * @param {string} key - The key to _privately_ retrieve the value of.
   */
  async privateRead(key: string): Promise<any> {
    this.ensureSpiral();

    if (Array.isArray(key)) {
      return (await this.performPrivateReads(key)).map(r => r.data);
    } else {
      const result = await this.performPrivateRead(key);
      return result ? result.data : null;
    }
  }

  /**
   * Privately reads the supplied key from the bucket, returning the value and
   * metadata corresponding to the key.
   *
   * No entity, including the Blyss service, should be able to determine which
   * key this method was called for.
   *
   * @param {string} key - The key to _privately_ retrieve the value of.
   */
  async privateReadWithMetadata(key: string): Promise<DataWithMetadata> {
    this.ensureSpiral();

    return await this.performPrivateRead(key);
  }

  /**
   * Privately intersects the given set of keys with the keys in this bucket,
   * returning the keys that intersected and their values. This is generally
   * slower than a single private read.
   *
   * No entity, including the Blyss service, should be able to determine which
   * keys this method was called for.
   *
   * The number of intersections could be determined by the Blyss service or a
   * network observer.
   *
   * @param keys - The keys to _privately_ intersect the value of.
   */
  async privateIntersect(keys: string[], retrieveValues = true): Promise<any> {
    this.ensureSpiral();

    if (keys.length < BLOOM_CUTOFF) {
      return (await this.performPrivateReads(keys)).map(x => x.data);
    }

    const bloomFilter = await this.api.bloom(this.name);
    const matches: string[] = [];
    for (const key of keys) {
      if (await bloomLookup(bloomFilter, key)) {
        matches.push(key);
      }
    }

    if (!retrieveValues) {
      return matches;
    }
    return (await this.performPrivateReads(matches)).map(x => x.data);
  }

  /**
   * Privately intersects the given set of keys with the keys in this bucket,
   * returning the keys that intersected. This is generally slower than a single
   * private read.
   *
   * No entity, including the Blyss service, should be able to determine which
   * keys this method was called for.
   *
   * @param keys - The keys to _privately_ intersect the value of.
   */
  async privateKeyIntersect(keys: string[]): Promise<string[]> {
    this.ensureSpiral();

    const bloomFilter = await this.api.bloom(this.name);
    const matches = [];
    for (const key of keys) {
      if (await bloomLookup(bloomFilter, key)) {
        matches.push(key);
      }
    }

    return matches;
  }

  /**
   * Privately checks if the given key is in the checklist-enabled bucket. This
   * method is only supported on special global buckets that are
   * checklist-enabled. The `supportsChecklistInclusion()` method returns a
   * boolean indicating support.
   *
   * @param key - The key to _privately_ check against the checklist.
   */
  async checkInclusion(key: string): Promise<boolean> {
    this.ensureDoublePIR();

    return await this.performDoublePIRReadBatch(key);
  }

  /** Returns whether this bucket supports `checkInclusion()` */
  supportsChecklistInclusion(): boolean {
    return this.scheme === 'doublepir';
  }

  /**
   * Serializes the state of the bucket client to a secret seed.
   *
   * This secret seed is sensitive! It must stay local to the client to preserve
   * query privacy.
   */
  toSecretSeed(): string {
    return this.secretSeed;
  }
}
