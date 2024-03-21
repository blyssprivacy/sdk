from typing import Any, Optional, Union
from . import bucket, api, seed

BLYSS_BUCKET_URL = "https://alpha.api.blyss.dev"
DEFAULT_BUCKET_PARAMETERS = {
    "maxItemSize": 1000,
    "keyStoragePolicy": "none",
    "version": 1,
}

ApiConfig = dict[str, str]


class BucketService:
    """A client to the hosted Blyss bucket service. Allows creation, deletion, and modification of buckets."""

    def __init__(self, api_key: str, endpoint: str = BLYSS_BUCKET_URL):
        """Initialize a client of the Blyss bucket service.

        Args:
            api_key: A valid Blyss API key.
            endpoint: A fully qualified endpoint URL for the bucket service, e.g. https://beta.api.blyss.dev.

        """
        self._api = api.API(api_key, endpoint)

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

    @staticmethod
    def _build_create_req(
        bucket_name: str, open_access: bool, usage_hints: dict[str, Any]
    ) -> dict[str, Any]:
        parameters = {**DEFAULT_BUCKET_PARAMETERS}
        parameters.update(usage_hints)
        bucket_create_req = {
            "name": bucket_name,
            "parameters": parameters,
            "open_access": open_access,
        }
        return bucket_create_req

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

        """
        bucket_create_req = self._build_create_req(
            bucket_name, open_access, usage_hints
        )
        self._api._blocking_create(bucket_create_req)

    def exists(self, name: str) -> bool:
        """Check if a bucket exists.

        Args:
            name: Bucket name to look up.

        Returns:
            True if a bucket with the given name currently exists.
        """
        return self._api._blocking_exists(name)

    def list_buckets(self) -> dict[str, Any]:
        """List all buckets accessible to this API key.

        Returns:
            A dictionary of bucket metadata, keyed by bucket name.
        """
        buckets = {}
        for b in self._api._blocking_list_buckets()["buckets"]:
            n = b.pop("name")
            buckets[n] = b
        return buckets


class BucketServiceAsync(BucketService):
    async def create(
        self,
        bucket_name: str,
        open_access: bool = False,
        usage_hints: dict[str, Any] = {},
    ):
        bucket_create_req = self._build_create_req(
            bucket_name, open_access, usage_hints
        )
        await self._api.create(bucket_create_req)

    async def connect(
        self,
        bucket_name: str,
        secret_seed: Optional[str] = None,
    ) -> bucket.AsyncBucket:
        """Returns an asynchronous client to the Blyss bucket. Identical functionality to `BucketService.connect`."""
        b = bucket.AsyncBucket(self._api, bucket_name, secret_seed=secret_seed)
        await b.async_init()
        return b

    async def exists(self, name: str) -> bool:
        return await self._api.exists(name)

    async def list_buckets(self) -> dict[str, Any]:
        buckets = {}
        for b in (await self._api.list_buckets())["buckets"]:
            n = b.pop("name")
            buckets[n] = b
        return buckets
