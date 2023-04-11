import { Api, ApiError } from '../client/api';
import { getRandomSeed } from '../client/seed';
import { Bucket } from './bucket';

export interface BucketParameters {
  /** The maximum item size this bucket supports */
  maxItemSize: MaxItemSizeIdentifier;
  keyStoragePolicy: kspIdentifier;
  version: number;
}

type MaxItemSizeIdentifier = '100B' | '1KB' | '10KB';
type kspIdentifier = 'none' | 'bloom' | 'full';

const DEFAULT_BUCKET_PARAMETERS: BucketParameters = {
  maxItemSize: '1KB',
  keyStoragePolicy: 'bloom',
  version: 1
};

const BLYSS_BUCKET_URL = 'https://beta.api.blyss.dev';
export const BLYSS_HINT_URL_PREFIX =
  'https://blyss-hints.s3.us-east-2.amazonaws.com/';
/** Information specifying how to connect to a Blyss bucket service API endpoint. */
export interface ApiConfig {
  /** A fully qualified endpoint URL for the bucket service. */
  endpoint?: string;
  /** An API key to supply with every request. */
  apiKey?: string;
}

/** A class representing a client to the Blyss bucket service. */
export class BucketService {
  apiConfig: ApiConfig;
  serviceEndpoint: string;
  api: Api;

  /**
   * Initialize a client of the Blyss bucket service.
   *
   * @param apiConfig - An API key, or an object containing an API
   *   configuration.
   */
  constructor(apiConfig: string | ApiConfig) {
    if (apiConfig === '<YOUR API KEY HERE>')
      throw new ApiError(500, '', '', 'You must supply an API key.');

    if (typeof apiConfig === 'string') {
      this.apiConfig = { apiKey: apiConfig };
    } else {
      this.apiConfig = apiConfig;
    }

    this.serviceEndpoint = BLYSS_BUCKET_URL;
    if (this.apiConfig.endpoint) {
      this.serviceEndpoint = this.apiConfig.endpoint;
    }

    this.api = new Api(this.apiConfig.apiKey, this.serviceEndpoint);
  }

  /**
   * Connect to an existing Blyss bucket.
   *
   * @param {string} bucketName - The name of the bucket to connect to.
   * @param {string} [secretSeed] - An optional secret seed to initialize the
   *   client using. The secret seed is used to encrypt client queries. If not
   *   supplied, a random one is generated.
   */
  async connect(bucketName: string, secretSeed?: string): Promise<Bucket> {
    let seed = getRandomSeed();
    if (secretSeed) {
      seed = secretSeed;
    }
    return await Bucket.initialize(this.api, bucketName, seed);
  }

  /**
   * Create a Blyss bucket.
   *
   * @param {string} bucketName - The bucket name. Bucket names must be 1-128
   *   characters long, composed of only lowercase letters (`[a-z]`), digits
   *   (`[0-9]`), and hyphens (`-`). If you want to share Blyss buckets across
   *   accounts, you can opt-in to the global Blyss namespace by prefixing your
   *   bucket name with `global.`. This is the only way in which the `.`
   *   character is allowed in bucket names.
   * @param {boolean} [openAccess] - If set to true, this bucket will be
   *   publicly readable by anyone. You should generally also make the bucket
   *   part of the global namespace when setting this option to true. Defaults
   *   to false.
   * @param {Partial<BucketParameters>} [params] - An optional object of
   *   requested parameters for the bucket. Defaults to `{}`. This is currently
   *   unused.
   */
  async create(bucketName: string, openAccess?: boolean, params?: any) {
    openAccess = openAccess || false;
    const parameters = { ...DEFAULT_BUCKET_PARAMETERS, ...params };
    const bucketCreateReq = {
      name: bucketName,
      parameters: JSON.stringify(parameters),
      open_access: openAccess
    };
    await this.api.create(JSON.stringify(bucketCreateReq));
  }

  /**
   * Check if a Blyss bucket exists.
   *
   * @param {string} bucketName - The bucket name.
   * @returns Whether a bucket with the given name exists.
   */
  async exists(bucketName: string): Promise<boolean> {
    try {
      await this.connect(bucketName);
      return true;
    } catch (e) {
      if (e instanceof ApiError && (e.status === 403 || e.status === 404)) {
        return false;
      } else {
        throw e;
      }
    }
    return true;
  }
}
