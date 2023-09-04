import { KeyInfo } from '../bucket/bucket';
import { BLYSS_HINT_URL_PREFIX } from '../bucket/bucket_service';
import { gzip } from '../compression/pako';
import { BloomFilter, bloomFilterFromBytes } from '../data/bloom';

const CREATE_PATH = '/create';
const MODIFY_PATH = '/modify';
const CLEAR_PATH = '/clear';
const DESTROY_PATH = '/destroy';
const CHECK_PATH = '/check';
const DELETE_PATH = '/delete';
const META_PATH = '/meta';
const BLOOM_PATH = '/bloom';
const LIST_KEYS_PATH = '/list-keys';
const SETUP_PATH = '/setup';
const HINT_PATH = '/hint';
const WRITE_PATH = '/write';
const READ_PATH = '/private-read';

export class ApiError extends Error {
  constructor(
    public status: number,
    public path: string,
    public body: string,
    public msg: string
  ) {
    super(msg);
    Object.setPrototypeOf(this, ApiError.prototype);
  }
}

export type BucketMetadata = any;

// HTTP utilities

async function getData(
  apiKey: string | null,
  url: string,
  getJson: boolean
): Promise<any | Uint8Array> {
  const headers = new Headers();
  if (apiKey) headers.append('X-API-Key', apiKey);

  const response = await fetch(url, {
    method: 'GET',
    headers
  });

  if (response.status < 200 || response.status > 299) {
    throw new ApiError(
      response.status,
      url,
      await response.text(),
      response.statusText
    );
  }

  if (getJson) {
    return response.json();
  } else {
    const data = await response.arrayBuffer();
    return new Uint8Array(data);
  }
}

async function postData(
  apiKey: string | null,
  url: string,
  data: Uint8Array | string,
  getJson: boolean
): Promise<Uint8Array | any> {
  const headers = new Headers();
  if (apiKey) headers.append('X-API-Key', apiKey);

  if (typeof data === 'string' || data instanceof String) {
    headers.append('Content-Type', 'application/json');
  } else {
    headers.append('Accept-Encoding', 'gzip');
    headers.append('Content-Encoding', 'gzip');
    headers.append('Content-Type', 'application/octet-stream');
    data = gzip(data);
  }

  const response = await fetch(url, {
    method: 'POST',
    body: data,
    headers
  });

  if (response.status < 200 || response.status > 299) {
    throw new ApiError(
      response.status,
      url,
      await response.text(),
      response.statusText
    );
  }

  if (getJson) {
    return response.json();
  } else {
    const data = await response.arrayBuffer();
    return new Uint8Array(data);
  }
}

async function postDataJson(
  apiKey: string | null,
  url: string,
  data: Uint8Array | string,
): Promise<any> {
  const headers = new Headers(
    {
      'Content-Type': 'application/json',
      'Accept-Encoding': 'gzip'
    }
  );
  if (apiKey) headers.append('X-API-Key', apiKey);

  // base64 encode bytes-like data
  if (typeof data !== 'string' && !(data instanceof String)) {
    data = btoa(String.fromCharCode.apply(null, data));
  }

  // compress
  data = gzip(data);
  headers.append('Content-Encoding', 'gzip');


  const response = await fetch(url, {
    method: 'POST',
    body: data,
    headers
  });

  if (response.status < 200 || response.status > 299) {
    throw new ApiError(
      response.status,
      url,
      await response.text(),
      response.statusText
    );
  }

  return response.json();
}


async function postFormData(
  url: string,
  fields: any,
  data: Uint8Array
): Promise<Uint8Array | any> {
  const formData = new FormData();
  for (const field in fields) {
    formData.append(field, fields[field]);
  }
  formData.append('file', new Blob([data]));

  const req = new Request(url, {
    method: 'POST',
    body: formData
  });
  const contentLength = (await req.clone().arrayBuffer()).byteLength;
  req.headers.append('Content-Length', contentLength + '');

  const response = await fetch(req);

  if (response.status < 200 || response.status > 299) {
    throw new ApiError(
      response.status,
      url,
      await response.text(),
      response.statusText
    );
  }
}

// API client

class Api {
  apiKey: string;
  serviceEndpoint: string;
  bucketEndpoint: string | undefined;

  constructor(
    apiKey: string,
    serviceEndpoint: string,
    bucketEndpoint?: string
  ) {
    this.apiKey = apiKey;
    this.serviceEndpoint = serviceEndpoint;
    this.bucketEndpoint = bucketEndpoint;
  }

  /**
   * Create an API client instance for a connection to a specific bucket.
   *
   * @param bucketEndpoint URL to a directly hosted bucket
   */
  static fromBucketEndpoint(bucketEndpoint: string): Api {
    return new this('', '', bucketEndpoint);
  }

  private serviceUrlFor(path: string): string {
    if (this.bucketEndpoint) {
      return this.bucketEndpoint + path;
    } else {
      return this.serviceEndpoint + path;
    }
  }

  private urlFor(bucketName: string, path: string): string {
    if (this.bucketEndpoint) {
      return this.bucketEndpoint + path;
    } else {
      return this.serviceEndpoint + '/' + bucketName + path;
    }
  }

  // Service methods

  /**
   * Create a new bucket, given the supplied data.
   *
   * @param dataJson A JSON-encoded string of the new bucket request.
   */
  async create(dataJson: string) {
    await postData(
      this.apiKey,
      this.serviceUrlFor(CREATE_PATH),
      dataJson,
      true
    );
  }

  /**
   * Check that a UUID is still valid on the server.
   *
   * @param uuid The UUID to check.
   */
  async check(uuid: string): Promise<any> {
    return await getData(
      this.apiKey,
      this.serviceUrlFor('/' + uuid + CHECK_PATH),
      true
    );
  }

  // Bucket-specific methods

  /**
   * Get metadata about a bucket.
   *
   * @param bucketName The name of the bucket.
   * @returns Metadata about the bucket.
   */
  async meta(bucketName: string): Promise<BucketMetadata> {
    return await getData(this.apiKey, this.urlFor(bucketName, META_PATH), true);
  }

  /**
   * Modify a bucket's properties.
   *
   * @param bucketName The name of the bucket.
   * @param dataJson A JSON-encoded string of the bucket metadata. Supports the same fields as `create()`.
   * @returns Bucket metadata after update.
   */
  async modify(bucketName: string, dataJson: string): Promise<BucketMetadata> {
    return await postData(this.apiKey, this.urlFor(bucketName, MODIFY_PATH), dataJson, true);
  }


  /**
   * Get the Bloom filter for keys in this bucket. The Bloom filter contains all
   * keys ever inserted into this bucket; it does not remove deleted keys.
   *
   * The false positive rate is determined by parameters chosen by the server.
   *
   * @param bucketName The name of the bucket.
   * @returns The Bloom filter for the keys of this bucket.
   */
  async bloom(bucketName: string): Promise<BloomFilter> {
    const presignedResp = await getData(
      this.apiKey,
      this.urlFor(bucketName, BLOOM_PATH),
      true
    );
    const data = await getData(null, presignedResp['url'], false);
    const filter = bloomFilterFromBytes(data);

    return filter;
  }

  /**
   * Lists all keys in a bucket.
   *
   * @param bucketName The name of the bucket.
   * @returns A list of information on every key in the bucket.
   */
  async listKeys(bucketName: string): Promise<KeyInfo[]> {
    return await getData(
      this.apiKey,
      this.urlFor(bucketName, LIST_KEYS_PATH),
      true
    );
  }

  /**
   * Upload new setup data.
   *
   * @param bucketName The name of the bucket associated with this setup data.
   * @param data The setup data.
   * @returns The setup data upload response, containing a UUID.
   */
  async setupS3(bucketName: string, data: Uint8Array): Promise<any> {
    if (this.bucketEndpoint) {
      return await postData(
        this.apiKey,
        this.urlFor(bucketName, SETUP_PATH),
        data,
        true
      );
    }

    const prelim_result = await postData(
      this.apiKey,
      this.urlFor(bucketName, SETUP_PATH),
      JSON.stringify({ length: data.length }),
      true
    );

    // perform the long upload
    await postFormData(prelim_result['url'], prelim_result['fields'], data);

    return prelim_result;
  }

  async setup(bucketName: string, data: Uint8Array): Promise<any> {
    return await postDataJson(this.apiKey, this.urlFor(bucketName, SETUP_PATH), data);
  }

  /**
   * Download hint data.
   *
   * @param bucketName The name of the bucket to get the hint data for.
   */
  async hint(bucketName: string): Promise<Uint8Array> {
    let url = BLYSS_HINT_URL_PREFIX + bucketName + '.hint';
    if (this.bucketEndpoint) {
      url = this.urlFor(bucketName, HINT_PATH);
    }
    const result = await getData(null, url, false);
    return result;
  }

  /** Destroy this bucket. */
  async destroy(bucketName: string) {
    await postData(
      this.apiKey,
      this.urlFor(bucketName, DESTROY_PATH),
      '',
      false
    );
  }

  /** Clear contents of this bucket. */
  async clear(bucketName: string) {
    await postData(
      this.apiKey,
      this.urlFor(bucketName, CLEAR_PATH),
      '',
      false
    );
  }

  /** Write to this bucket. */
  async write(bucketName: string, data: Uint8Array) {
    await postData(
      this.apiKey,
      this.urlFor(bucketName, WRITE_PATH),
      data,
      false
    );
  }

  /** Delete a key in this bucket. */
  async deleteKey(bucketName: string, key: string) {
    await postData(
      this.apiKey,
      this.urlFor(bucketName, DELETE_PATH),
      new TextEncoder().encode(key),
      false
    );
  }

  /** Privately read data from this bucket. */
  async privateRead(bucketName: string, data: Uint8Array): Promise<Uint8Array> {
    return await postData(
      this.apiKey,
      this.urlFor(bucketName, READ_PATH),
      data,
      false
    );
  }

  /** Privately read data from this bucket. */
  async privateReadMultipart(
    bucketName: string,
    data: Uint8Array,
    targetUrl?: string
  ): Promise<Uint8Array> {
    if (!targetUrl) targetUrl = this.urlFor(bucketName, READ_PATH);

    if (this.bucketEndpoint) {
      return await postData(
        this.apiKey,
        this.urlFor(bucketName, SETUP_PATH),
        data,
        true
      );
    }

    const prelim_result = await postData(this.apiKey, targetUrl, '', true);

    // perform the long upload
    await postFormData(prelim_result['url'], prelim_result['fields'], data);

    return await postData(
      this.apiKey,
      targetUrl,
      JSON.stringify({ uuid: prelim_result['uuid'] }),
      false
    );
  }
}

export { Api };
