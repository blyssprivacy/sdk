"""API

INTERNAL

Abstracts all functionality offered by Blyss services.
"""

from typing import Any, Optional, Union
import httpx
import gzip
import json
import logging
import base64
import asyncio

from blyss.bloom import BloomFilter

CREATE_PATH = "/create"
MODIFY_PATH = "/modify"
DESTROY_PATH = "/destroy"
CLEAR_PATH = "/clear"
CHECK_PATH = "/check"
LIST_BUCKETS_PATH = "/list-buckets"
DELETE_PATH = "/delete"
META_PATH = "/meta"
BLOOM_PATH = "/bloom"
LIST_KEYS_PATH = "/list-keys"
SETUP_PATH = "/setup"
WRITE_PATH = "/write"
READ_PATH = "/private-read"

APIGW_MAX_SIZE = 6e6 / (4 / 3) * 0.95  # 6MB, base64 encoded, plus 5% margin
_GLOBAL_ENABLE_REQUEST_COMPRESSION = False


# Not compatible with nested asyncio loops.
# If the caller is running in an asyncio context, use the async methods directly.
def async_runner(func, *args, **kwargs):
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    try:
        # Run the async function and get the result
        result = loop.run_until_complete(func(*args, **kwargs))
        return result
    finally:
        loop.close()


class ApiException(Exception):
    """Exception raised when an API call to the Blyss service fails."""

    def __init__(self, message: str, code: int):
        self.message = message
        """Error message returned by the server."""
        self.code = code
        """HTTP status code returned by the server."""
        super().__init__(message)


def _check_http_error(r: httpx.Response):
    """Throws an ApiException with message on any unsuccessful HTTP response."""
    status_code = r.status_code
    if status_code < 200 or status_code > 299:
        try:
            errmsg = r.text
        except:
            errmsg = f"<undecodable response body, size {len(r.content)} bytes>"
        raise ApiException(
            errmsg,
            status_code,
        )


async def _async_get(api_key: Optional[str], url: str) -> Any:
    headers = {}
    if api_key:
        headers["x-api-key"] = api_key

    logging.info(f"GET {url} {headers}")
    async with httpx.AsyncClient() as client:
        r = await client.get(url, headers=headers)
    _check_http_error(r)

    return r.json()


async def _async_post_data(
    api_key: str, url: str, data: Union[bytes, Any], compress: bool = True
) -> Any:
    """Perform an async HTTP POST request, returning a JSON-parsed dict response"""
    headers = {"x-api-key": api_key, "Content-Type": "application/json"}

    if data is None:
        payload = None
    else:
        if type(data) == bytes:
            data_jsonable = base64.b64encode(data).decode("utf-8")
        else:
            data_jsonable = data
        data_json = json.dumps(data_jsonable).encode("utf-8")

        if len(data_json) > APIGW_MAX_SIZE:
            raise ValueError(
                f"Request data is too large ({len(data_json)} JSON bytes); maximum size is {APIGW_MAX_SIZE} bytes"
            )

        if compress and _GLOBAL_ENABLE_REQUEST_COMPRESSION:
            # apply gzip compression to data before sending
            payload = gzip.compress(data_json)
            headers["Content-Encoding"] = "gzip"
        else:
            payload = data_json

    async with httpx.AsyncClient(timeout=httpx.Timeout(5, read=None)) as client:
        r = await client.post(url, content=payload, headers=headers)

    _check_http_error(r)
    return r.json()


class API:
    """
    A class representing the functionality exposed by the Blyss bucket service.
    """

    def __init__(self, api_key: str, service_endpoint: str):
        """Create a new instance of the ServiceAPI interface object.

        Args:
            api_key (str): A key to be passed in all requests in the "x-api-key" header.
            service_endpoint (str): A fully-qualified URL to send requests to. There should be
            no trailing slash.
        """
        self.api_key = api_key
        self.service_endpoint = service_endpoint

    # Service methods

    def _service_url_for(self, path: str) -> str:
        return self.service_endpoint + path

    async def create(self, data_jsonable: dict) -> dict[Any, Any]:
        """Create a new bucket, given the supplied data.

        Args:
            data_json (str): A JSON-encoded string of the new bucket request.
        """
        return await _async_post_data(
            self.api_key,
            self._service_url_for(CREATE_PATH),
            data_jsonable,
        )

    def _blocking_create(self, *args, **kwargs):
        return async_runner(self.create, *args, **kwargs)

    async def check(self, uuid: str) -> bool:
        """Check that a UUID is still valid on the server.

        Args:
            uuid (str): The UUID to check
        """

        try:
            await _async_get(
                self.api_key, self._service_url_for("/" + uuid + CHECK_PATH)
            )
            return True
        except ApiException as e:
            if e.code == 404:
                return False
            else:
                raise e

    def _blocking_check(self, *args, **kwargs):
        return async_runner(self.check, *args, **kwargs)

    async def exists(self, bucket_name: str) -> bool:
        """Check if a bucket exists.

        Args:
            bucket_name (str): The name of the bucket to check
        """
        try:
            await _async_get(
                self.api_key, self._service_url_for("/" + bucket_name + CHECK_PATH)
            )
            return True
        except ApiException as e:
            if e.code == 404:
                return False
            else:
                raise e

    def _blocking_exists(self, *args, **kwargs):
        return async_runner(self.exists, *args, **kwargs)

    async def list_buckets(self) -> dict[Any, Any]:
        """List all buckets accessible to this API key.

        Returns:
            dict: A JSON-parsed dict of all buckets accessible to this API key.
        """
        return await _async_get(self.api_key, self._service_url_for(LIST_BUCKETS_PATH))

    def _blocking_list_buckets(self, *args, **kwargs):
        return async_runner(self.list_buckets, *args, **kwargs)

    # Bucket-specific methods
    def _url_for(self, bucket_name: str, path: str) -> str:
        return self.service_endpoint + "/" + bucket_name + path

    async def modify(self, bucket_name: str, data_jsonable: Any) -> dict[Any, Any]:
        """Modify existing bucket.

        Args:
            data_json (str): same as create.
        """
        return await _async_post_data(
            self.api_key, self._url_for(bucket_name, MODIFY_PATH), data_jsonable
        )

    def _blocking_modify(self, *args, **kwargs):
        return async_runner(self.modify, *args, **kwargs)

    async def meta(self, bucket_name: str) -> dict[Any, Any]:
        """Get metadata about a bucket.

        Returns:
            dict: Metadata about a bucket.
        """
        return await _async_get(self.api_key, self._url_for(bucket_name, META_PATH))

    def _blocking_meta(self, *args, **kwargs):
        return async_runner(self.meta, *args, **kwargs)

    async def bloom(self, bucket_name: str) -> BloomFilter:
        """Get the Bloom filter for keys in this bucket. The Bloom filter contains all
        keys ever inserted into this bucket; it does not remove deleted keys.

        The false positive rate is determined by parameters chosen by the server.

        Returns:
            BloomFilter: A Bloom filter for keys in the bucket.
        """
        r = await _async_get(self.api_key, self._url_for(bucket_name, BLOOM_PATH))
        presigned_url = r["url"]

        raw_bloom_filter = await _async_get(None, presigned_url)
        bloom_filter = BloomFilter.from_bytes(raw_bloom_filter)

        return bloom_filter

    def _blocking_bloom(self, *args, **kwargs):
        return async_runner(self.bloom, *args, **kwargs)

    async def setup(self, bucket_name: str, data: bytes) -> str:
        """Upload new setup data.

        Args:
            data (bytes): Setup data to upload.
        """
        resp = await _async_post_data(
            self.api_key, self._url_for(bucket_name, SETUP_PATH), data
        )

        return resp["uuid"]

    def _blocking_setup(self, *args, **kwargs):
        return async_runner(self.setup, *args, **kwargs)

    async def destroy(self, bucket_name: str):
        """Destroy this bucket."""
        await _async_post_data(
            self.api_key, self._url_for(bucket_name, DESTROY_PATH), data=None
        )

    def _blocking_destroy(self, *args, **kwargs):
        return async_runner(self.destroy, *args, **kwargs)

    async def clear(self, bucket_name: str):
        """Delete all keys in this bucket."""
        await _async_post_data(
            self.api_key, self._url_for(bucket_name, CLEAR_PATH), data=None
        )

    def _blocking_clear(self, *args, **kwargs):
        return async_runner(self.clear, *args, **kwargs)

    async def write(self, bucket_name: str, data_jsonable: dict[str, Optional[str]]):
        """Write JSON payload to this bucket."""
        return await _async_post_data(
            self.api_key,
            self._url_for(bucket_name, WRITE_PATH),
            data_jsonable,
            compress=True,
        )

    def _blocking_write(self, *args, **kwargs):
        async_runner(self.write, *args, **kwargs)

    async def private_read(
        self, bucket_name: str, queries: list[bytes]
    ) -> list[Optional[bytes]]:
        """Privately read data from this bucket."""
        data_jsonable = [base64.b64encode(q).decode("utf-8") for q in queries]
        r: list[str] = await _async_post_data(
            self.api_key,
            self._url_for(bucket_name, READ_PATH),
            data_jsonable,
            compress=True,
        )
        return [base64.b64decode(v) if v is not None else None for v in r]

    def _blocking_private_read(self, *args, **kwargs):
        return async_runner(self.private_read, *args, **kwargs)
