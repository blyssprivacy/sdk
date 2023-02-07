import { seedFromString } from '../client/seed';

import {
  ApiClient,
  decode_response,
  extract_result,
  generate_keys,
  generate_query,
  get_row,
  initialize_client
} from './helper';

export class BlyssLib {
  private innerClient: ApiClient;
  private secretSeed: string;

  generateKeys(generatePublicParameters: boolean) {
    return generate_keys(
      this.innerClient,
      seedFromString(this.secretSeed),
      generatePublicParameters
    );
  }

  getRow(key: string): number {
    return get_row(this.innerClient, key);
  }

  generateQuery(uuid: string, rowIdx: number) {
    return generate_query(this.innerClient, uuid, rowIdx);
  }

  decodeResponse(response: Uint8Array): Uint8Array {
    return decode_response(this.innerClient, response);
  }

  extractResult(key: string, data: Uint8Array): Uint8Array {
    return extract_result(this.innerClient, key, data);
  }

  free() {
    this.innerClient.free();
    this.innerClient = null;
    this.secretSeed = '';
  }

  constructor(params: string, secretSeed: string) {
    this.innerClient = initialize_client(params);
    this.secretSeed = secretSeed;
  }
}
