from typing import Any, Optional, Union
from . import bucket, api, seed
import json

BLYSS_BUCKET_URL = "https://beta.api.blyss.dev"
DEFAULT_BUCKET_PARAMETERS = {"maxItemSize": 1000}

ApiConfig = dict[str, str]


class BucketService:
    """A class representing a client to the Blyss bucket service."""

    def __init__(self, api_config: Union[str, ApiConfig]):
        """Initialize a client of the Blyss bucket service.

        Args:
            api_config (Union[str, ApiConfig]): An API key, or
            a dictionary containing an API configuration.
            The minimum set of keys is:
                "endpoint": A fully qualified endpoint URL for the bucket service.
                "api_key" : An API key to supply with every request.
        """
        if isinstance(api_config, str):
            api_config = {"api_key": api_config}
        self.api_config = api_config

        self.service_endpoint = BLYSS_BUCKET_URL
        if "endpoint" in api_config:
            self.service_endpoint = api_config["endpoint"]

        self.api = api.API(self.api_config["api_key"], self.service_endpoint)

    def connect(
        self,
        bucket_name: str,
        secret_seed: Optional[str] = None,
    ) -> bucket.Bucket:
        """Connect to an existing Blyss bucket.

        Args:
            bucket_name (str): The name of the bucket to connect to.
            secret_seed (Optional[str], optional): An optional secret seed to
            initialize the client using. The secret seed is used to encrypt
            client queries. If not supplied, a random one is generated with `os.urandom`.

        Returns:
            bucket.Bucket: An object representing a client to the Blyss bucket.
        """
        if secret_seed is None:
            secret_seed = seed.get_random_seed()
        b = bucket.Bucket(self.api, bucket_name, secret_seed=secret_seed)
        return b

    def connect_async(
        self, bucket_name: str, secret_seed: Optional[str] = None
    ) -> bucket.AsyncBucket:
        """Connect to an existing Blyss bucket, using an asyncio-ready interface.

        Args:
            see connect()

        Returns:
            bucket.Bucket: An object representing a client to the Blyss bucket.
        """
        return bucket.AsyncBucket(self.api, bucket_name, secret_seed=secret_seed)

    def create(
        self,
        bucket_name: str,
        open_access: bool = False,
        usage_hints: dict[Any, Any] = {},
    ):
        """Create a new Blyss bucket.

        Args:
            bucket_name (str): The bucket name. See sanitize_bucket_name for naming rules.
            open_access (bool): If True, bucket will support open read-only access.
            usage_hints (dict): A dictionary of hints describing the intended usage of this bucket.
                                The Blyss service will optimize the encryption scheme accordingly.
        """
        parameters = {**DEFAULT_BUCKET_PARAMETERS, **usage_hints}
        bucket_create_req = {
            "name": bucket_name,
            "parameters": json.dumps(parameters),
            "open_access": open_access,
        }
        self.api.create(json.dumps(bucket_create_req))

    def exists(self, name: str) -> bool:
        """Checks if a bucket exists.

        Args:
            bucket_name (str): The bucket name.

        Returns:
            bool: Whether a bucket with the given name already exists.
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
        for b in self.api.list_buckets()["buckets"]:
            n = b.pop("name")
            buckets[n] = b
        return buckets
