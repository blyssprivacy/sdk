"""API

INTERNAL

Abstracts all functionality offered by Blyss services.
"""

from typing import Any, Optional, Union
import requests
import httpx
import gzip
import asyncio
import json
import logging
import base64

from blyss.req_compression import get_session
from blyss.bloom import BloomFilter

CREATE_PATH = "/create"
DESTROY_PATH = "/destroy"
CHECK_PATH = "/check"
LIST_BUCKETS_PATH = "/list-buckets"
DELETE_PATH = "/delete"
META_PATH = "/meta"
BLOOM_PATH = "/bloom"
LIST_KEYS_PATH = "/list-keys"
SETUP_PATH = "/setup"
WRITE_PATH = "/write"
READ_PATH = "/private-read"


class ApiException(Exception):
    def __init__(self, message: str, code: int):
        """Initialize ApiException with message."""
        self.message = message
        self.code = code
        super().__init__(message)


def _check_http_error(resp: requests.Response):
    """Throws an ApiException with message on any unsuccessful HTTP response."""
    status_code = resp.status_code
    if status_code < 200 or status_code > 299:
        raise ApiException(
            f"Request failed, with unsuccessful HTTP status code {status_code} and message '{resp.content}'",
            status_code,
        )


def _get_data(api_key: Optional[str], url: str) -> bytes:
    """Perform an HTTP GET request, returning bytes"""
    headers = {}
    if api_key:
        headers["x-api-key"] = api_key

    logging.info(f"GET {url} {headers}")
    resp = requests.get(url, headers=headers)
    _check_http_error(resp)

    return resp.content


async def _async_get_data(
    api_key: Optional[str], url: str, decode_json: bool = True
) -> Any:
    headers = {}
    if api_key:
        headers["x-api-key"] = api_key

    logging.info(f"GET {url} {headers}")
    async with httpx.AsyncClient() as client:
        r = await client.get(url, headers=headers)
    _check_http_error(r)

    if decode_json:
        return r.json()
    else:
        return r.content


def _get_data_json(api_key: str, url: str) -> dict[Any, Any]:
    """Perform an HTTP GET request, returning a JSON-parsed dict"""
    return json.loads(_get_data(api_key, url))


def _post_data(api_key: str, url: str, data: Union[bytes, str]) -> bytes:
    """Perform an HTTP POST request."""
    headers = {}
    if api_key:
        headers["x-api-key"] = api_key
    if type(data) == bytes:
        headers["Content-Type"] = "application/octet-stream"

    logging.info(f"POST {url} (length: {len(data)} bytes)")
    resp = None
    if type(data) == bytes:
        resp = get_session().post(url, data, headers=headers)
    else:
        resp = requests.post(url, data, headers=headers)
    _check_http_error(resp)

    return resp.content


def _post_data_json(api_key: str, url: str, data: Union[bytes, str]) -> dict[Any, Any]:
    """Perform an HTTP POST request, returning a JSON-parsed dict"""
    return json.loads(_post_data(api_key, url, data))


def _post_form_data(url: str, fields: dict[Any, Any], data: bytes):
    """Perform a multipart/form-data POST"""
    files = {"file": data}
    resp = requests.post(url, data=fields, files=files)
    _check_http_error(resp)


async def _async_post_data(
    api_key: str,
    url: str,
    data: Union[str, bytes],
    compress: bool = True,
    decode_json: bool = True,
) -> Any:
    """Perform an async HTTP POST request, returning a JSON-parsed dict response"""
    headers = {
        "x-api-key": api_key,
    }
    if type(data) == str:
        headers["Content-Type"] = "application/json"
        data = data.encode("utf-8")
    else:
        headers["Content-Type"] = "application/octet-stream"
    assert type(data) == bytes

    if compress:
        # apply gzip compression to data before sending
        data = gzip.compress(data)
        headers["Content-Encoding"] = "gzip"

    async with httpx.AsyncClient(timeout=httpx.Timeout(5, read=None)) as client:
        r = await client.post(url, content=data, headers=headers)

    _check_http_error(r)  # type: ignore
    if decode_json:
        return r.json()
    else:
        return r.content


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

    def create(self, data_json: str) -> dict[Any, Any]:
        """Create a new bucket, given the supplied data.

        Args:
            data_json (str): A JSON-encoded string of the new bucket request.
        """
        return _post_data_json(
            self.api_key, self._service_url_for(CREATE_PATH), data_json
        )

    def check(self, uuid: str) -> dict[Any, Any]:
        """Check that a UUID is still valid on the server.

        Args:
            uuid (str): The UUID to check
        """
        return _get_data_json(
            self.api_key, self._service_url_for("/" + uuid + CHECK_PATH)
        )

    async def async_check(self, uuid: str) -> dict[Any, Any]:
        return await _async_get_data(
            self.api_key,
            self._service_url_for("/" + uuid + CHECK_PATH),
            decode_json=True,
        )

    def list_buckets(self) -> dict[Any, Any]:
        """List all buckets accessible to this API key.

        Returns:
            dict: A JSON-parsed dict of all buckets accessible to this API key.
        """
        return _get_data_json(self.api_key, self._service_url_for(LIST_BUCKETS_PATH))

    # Bucket-specific methods
    def _url_for(self, bucket_name: str, path: str) -> str:
        return self.service_endpoint + "/" + bucket_name + path

    def meta(self, bucket_name: str) -> dict[Any, Any]:
        """Get metadata about a bucket.

        Returns:
            dict: Metadata about a bucket.
        """
        return _get_data_json(self.api_key, self._url_for(bucket_name, META_PATH))

    def bloom(self, bucket_name: str) -> BloomFilter:
        """Get the Bloom filter for keys in this bucket. The Bloom filter contains all
        keys ever inserted into this bucket; it does not remove deleted keys.

        The false positive rate is determined by parameters chosen by the server.

        Returns:
            BloomFilter: A Bloom filter for keys in the bucket.
        """
        presigned_url = _get_data_json(
            self.api_key, self._url_for(bucket_name, BLOOM_PATH)
        )["url"]

        raw_bloom_filter = _get_data(None, presigned_url)
        bloom_filter = BloomFilter.from_bytes(raw_bloom_filter)

        return bloom_filter

    def setup(self, bucket_name: str, data: bytes) -> dict[Any, Any]:
        """Upload new setup data.

        Args:
            data (bytes): Setup data to upload.
        """
        prelim_result = _post_data_json(
            self.api_key,
            self._url_for(bucket_name, SETUP_PATH),
            json.dumps({"length": len(data)}),
        )

        _post_form_data(prelim_result["url"], prelim_result["fields"], data)

        return prelim_result

    def list_keys(self, bucket_name: str) -> dict[str, Any]:
        """List all keys in this bucket."""
        return _get_data_json(self.api_key, self._url_for(bucket_name, LIST_KEYS_PATH))

    def destroy(self, bucket_name: str):
        """Destroy this bucket."""
        _post_data(self.api_key, self._url_for(bucket_name, DESTROY_PATH), "")

    def write(self, bucket_name: str, data: bytes):
        """Write some data to this bucket."""
        _post_data(self.api_key, self._url_for(bucket_name, WRITE_PATH), data)

    async def async_write(self, bucket_name: str, data: str):
        """Write JSON payload to this bucket."""
        await _async_post_data(
            self.api_key, self._url_for(bucket_name, WRITE_PATH), data, decode_json=True
        )

    def delete_key(self, bucket_name: str, key: str):
        """Delete a key in this bucket."""
        _post_data(
            self.api_key, self._url_for(bucket_name, DELETE_PATH), key.encode("utf-8")
        )

    def private_read(self, bucket_name: str, data: bytes) -> bytes:
        """Privately read data from this bucket."""
        val = _post_data(self.api_key, self._url_for(bucket_name, READ_PATH), data)
        return base64.b64decode(val)

    async def async_private_read(self, bucket_name: str, data: bytes) -> bytes:
        """Privately read data from this bucket."""
        val: bytes = await _async_post_data(
            self.api_key, self._url_for(bucket_name, READ_PATH), data, decode_json=False
        )
        # AWS APIGW encodes its responses as base64
        return base64.b64decode(val)
        # return self.private_read(bucket_name, data)
