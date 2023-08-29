from typing import Any, Optional, Union
from . import bucket, api, seed
import json

BLYSS_BUCKET_URL = "https://beta.api.blyss.dev"
DEFAULT_BUCKET_PARAMETERS = {
    "maxItemSize": 1000,
    "keyStoragePolicy": "none",
    "version": 1,
}

ApiConfig = dict[str, str]


class BucketService:
    """A client to the hosted Blyss bucket service. Allows creation, deletion, and modification of buckets."""

    def __init__(self, api_config: Union[str, ApiConfig]):
        """Initialize a client of the Blyss bucket service.

        Args:
            api_config: An API key string, or
            a dictionary containing an API configuration.
            The minimum set of keys is:
                "endpoint": A fully qualified endpoint URL for the bucket service.
                "api_key" : An API key to supply with every request.
        """
        if isinstance(api_config, str):
            api_config = {"api_key": api_config}

        service_endpoint = api_config.get("endpoint", BLYSS_BUCKET_URL)
        self._api = api.API(api_config["api_key"], service_endpoint)

    def connect(
        self,
        bucket_name: str,
        secret_seed: Optional[str] = None,
    ) -> bucket.Bucket:
        """Connect to an existing Blyss bucket.

        Args:
            bucket_name: The name of the bucket to connect to.
            secret_seed: An optional secret seed to derive the client secret,
                         which will be used to encrypt all client queries.
                         If not supplied, a random one is generated with `os.urandom`.

        Returns:
            An object representing a client to the Blyss bucket.
        """
        if secret_seed is None:
            secret_seed = seed.get_random_seed()
        b = bucket.Bucket(self._api, bucket_name, secret_seed=secret_seed)
        return b

    def connect_async(
        self, bucket_name: str, secret_seed: Optional[str] = None
    ) -> bucket.AsyncBucket:
        """Returns an asynchronous client to the Blyss bucket. Identical functionality to `connect`."""
        return bucket.AsyncBucket(self._api, bucket_name, secret_seed=secret_seed)

    def create(
        self,
        bucket_name: str,
        open_access: bool = False,
        usage_hints: dict[str, Any] = {},
    ):
        """Create a new Blyss bucket.

        Args:
            bucket_name: Name of the new bucket. See [bucket naming rules](https://docs.blyss.dev/docs/buckets#names).
            open_access: If True, bucket will support open read-only access, i.e. any user can perform reads. See [open access permissions](https://docs.blyss.dev/docs/buckets#permissions).
            usage_hints: A dictionary of hints describing the intended usage of this bucket. Supported keys:
                            - "maxItemSize": The maximum size of any item in the bucket, in bytes.
                            A scheme will be chosen that can support at least this size, and possibly more.
                            Larger item sizes carry performance costs; expect longer query times and more bandwidth usage.
                            - "keyStoragePolicy": The key storage policy to use for this bucket. Options:
                                - "none" (default): Stores no key-related information. This is the most performant option and will maximize write speed.
                                - "bloom": Enables `Bucket.private_intersect()`. Uses a bloom filter to store probablistic information of key membership, with minimal impact on write speed.
                                - "full": Store all keys in full. Enables `Bucket.list_keys()`. Will result in significantly slower writes.

        """
        parameters = {**DEFAULT_BUCKET_PARAMETERS}
        parameters.update(usage_hints)
        bucket_create_req = {
            "name": bucket_name,
            "parameters": json.dumps(parameters),
            "open_access": open_access,
        }
        self._api.create(json.dumps(bucket_create_req))

    def exists(self, name: str) -> bool:
        """Check if a bucket exists.

        Args:
            name: Bucket name to look up.

        Returns:
            True if a bucket with the given name currently exists.
        """
        try:
            self.connect(name)
            return True
        except api.ApiException as e:
            if e.code in [403, 404]:  # Forbidden (need read permission to see metadata)
                # or Not Found (bucket of this name doesn't exist)
                return False
            else:
                raise e

    def list_buckets(self) -> dict[str, Any]:
        """List all buckets accessible to this API key.

        Returns:
            A dictionary of bucket metadata, keyed by bucket name.
        """
        buckets = {}
        for b in self._api.list_buckets()["buckets"]:
            n = b.pop("name")
            buckets[n] = b
        return buckets
