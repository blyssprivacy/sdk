import { Bucket } from './bucket/bucket';
import type { KeyInfo } from './bucket/bucket';
import type { ApiConfig } from './bucket/bucket_service';
import { BucketService } from './bucket/bucket_service';
import type { ApiError } from './client/api';
import { DataWithMetadata } from './data/serializer';

export { BucketService as Client, Bucket };

export type { KeyInfo, BucketService, ApiError, ApiConfig, DataWithMetadata };

// External copyright notices:
/*! pako (C) 1995-2013 Jean-loup Gailly and Mark Adler */
/*! pako (C) 2014-2017 Vitaly Puzrin and Andrey Tupitsin */
