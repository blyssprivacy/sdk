"""API

INTERNAL

Abstracts all functionality offered by Blyss services.
"""

from typing import Any, Optional, Union
import requests
import httpx
import gzip
import json
import logging
import base64

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


class ApiException(Exception):
    """Exception raised when an API call to the Blyss service fails."""

    def __init__(self, message: str, code: int):
        self.message = message
        """Error message returned by the server."""
        self.code = code
        """HTTP status code returned by the server."""
        super().__init__(message)


def _check_http_error(resp: requests.Response | httpx.Response):
    """Throws an ApiException with message on any unsuccessful HTTP response."""
    status_code = resp.status_code
    if status_code < 200 or status_code > 299:
        try:
            errmsg = resp.text
        except:
            errmsg = f"<undecodable response body, size {len(resp.content)} bytes>"
        raise ApiException(
            errmsg,
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
        # compress data before sending
        zdata = gzip.compress(data)
        headers["Content-Encoding"] = "gzip"
        resp = requests.post(url, zdata, headers=headers)
    else:
        resp = requests.post(url, data, headers=headers)
    _check_http_error(resp)

    return resp.content


def _post_data_json(api_key: str, url: str, data: Union[bytes, Any]) -> Any:
    """Perform an HTTP POST request, returning a JSON-parsed dict.
    Request data can be any JSON string, or a raw bytestring that will be base64-encoded before send.
    All requests and responses are compressed JSON."""

    if len(data) > APIGW_MAX_SIZE:
        raise ValueError(
            f"Request data is too large ({len(data)} bytes); maximum size is {APIGW_MAX_SIZE} bytes"
        )

    c = httpx.Client(
        headers={
            "x-api-key": api_key,
            "Content-Type": "application/json",
        }
    )

    if type(data) == bytes:
        data_jsonable = base64.b64encode(data)
    else:
        data_jsonable = data
    data_json = json.dumps(data_jsonable).encode("utf-8")

    # compress requests larger than 1KB
    extra_headers = {}
    if len(data_json) > 1000:
        payload = gzip.compress(data_json)
        extra_headers["Content-Encoding"] = "gzip"
    else:
        payload = data_json

    resp = c.post(url, content=payload, headers=extra_headers)

    return resp.json()


def _post_form_data(url: str, fields: dict[Any, Any], data: bytes):
    """Perform a multipart/form-data POST"""
    files = {"file": data}
    resp = requests.post(url, data=fields, files=files)
    _check_http_error(resp)


async def _async_post_data(
    api_key: str,
    url: str,
    data: Union[bytes, Any],
    compress: bool = True,
    decode_json: bool = True,
) -> Any:
    """Perform an async HTTP POST request, returning a JSON-parsed dict response"""
    headers = {"x-api-key": api_key, "Content-Type": "application/json"}

    if type(data) == bytes:
        data_jsonable = base64.b64encode(data).decode("utf-8")
    else:
        data_jsonable = data
    data_json = json.dumps(data_jsonable).encode("utf-8")

    if compress:
        # apply gzip compression to data before sending
        payload = gzip.compress(data_json)
        headers["Content-Encoding"] = "gzip"
    else:
        payload = data_json

    async with httpx.AsyncClient(timeout=httpx.Timeout(5, read=None)) as client:
        r = await client.post(url, content=payload, headers=headers)

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

    def create(self, data_jsonable: dict) -> dict[Any, Any]:
        """Create a new bucket, given the supplied data.

        Args:
            data_json (str): A JSON-encoded string of the new bucket request.
        """
        return _post_data_json(
            self.api_key, self._service_url_for(CREATE_PATH), data_jsonable
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

    def modify(self, bucket_name: str, data_jsonable: Any) -> dict[Any, Any]:
        """Modify existing bucket.

        Args:
            data_json (str): same as create.
        """
        return _post_data_json(
            self.api_key, self._url_for(bucket_name, MODIFY_PATH), data_jsonable
        )

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

    def setup(self, bucket_name: str, data: bytes) -> str:
        """Upload new setup data.

        Args:
            data (bytes): Setup data to upload.
        """
        if len(data) > APIGW_MAX_SIZE:
            raise ValueError(
                f"Scheme public parameters too large ({len(data)} bytes); maximum size is {APIGW_MAX_SIZE} bytes"
            )

        r = _post_data_json(self.api_key, self._url_for(bucket_name, SETUP_PATH), data)

        return r["uuid"]

    async def async_setup(self, bucket_name: str, data: bytes) -> str:
        resp = await _async_post_data(
            self.api_key,
            self._url_for(bucket_name, SETUP_PATH),
            data,
            compress=True,
            decode_json=True,
        )

        return resp["uuid"]

    def list_keys(self, bucket_name: str) -> list[str]:
        """List all keys in this bucket."""
        return _get_data_json(self.api_key, self._url_for(bucket_name, LIST_KEYS_PATH))  # type: ignore

    def destroy(self, bucket_name: str):
        """Destroy this bucket."""
        _post_data(self.api_key, self._url_for(bucket_name, DESTROY_PATH), "")

    def clear(self, bucket_name: str):
        """Delete all keys in this bucket."""
        _post_data(self.api_key, self._url_for(bucket_name, CLEAR_PATH), "")

    def write(self, bucket_name: str, data: dict[str, Optional[bytes]]):
        """Write some data to this bucket."""
        data_jsonable = {
            k: None if v is None else base64.b64encode(v).decode("utf-8")
            for k, v in data.items()
        }
        _post_data_json(
            self.api_key, self._url_for(bucket_name, WRITE_PATH), data_jsonable
        )

    async def async_write(self, bucket_name: str, data: dict[str, Optional[bytes]]):
        """Write JSON payload to this bucket."""

        data_jsonable = {
            k: None if v is None else base64.b64encode(v).decode("utf-8")
            for k, v in data.items()
        }

        await _async_post_data(
            self.api_key,
            self._url_for(bucket_name, WRITE_PATH),
            data_jsonable,
            compress=True,
        )

    def delete_key(self, bucket_name: str, key: str):
        """Delete a key in this bucket."""
        _post_data(
            self.api_key, self._url_for(bucket_name, DELETE_PATH), key.encode("utf-8")
        )

    def private_read(
        self, bucket_name: str, queries: list[bytes]
    ) -> list[Optional[bytes]]:
        """Privately read data from this bucket."""
        data_jsonable = [base64.b64encode(q).decode("utf-8") for q in queries]
        r = _post_data_json(
            self.api_key, self._url_for(bucket_name, READ_PATH), data_jsonable
        )
        return [base64.b64decode(v) if v is not None else None for v in r]

    async def async_private_read(
        self, bucket_name: str, queries: list[bytes]
    ) -> list[Optional[bytes]]:
        """Privately read data from this bucket."""
        data_jsonable = [base64.b64encode(q).decode("utf-8") for q in queries]
        r: list[str] = await _async_post_data(
            self.api_key,
            self._url_for(bucket_name, READ_PATH),
            data_jsonable,
            compress=True,
            decode_json=True,
        )
        return [base64.b64decode(v) if v is not None else None for v in r]
