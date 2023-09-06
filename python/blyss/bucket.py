"""Bucket

Abstracts functionality on an existing bucket.
"""

from typing import Optional, Any, Union, Iterator

from . import api, seed
from .blyss_lib import BlyssLib

import json
import bz2
import asyncio
import base64


class Bucket:
    """Interface to a single Blyss bucket."""

    name: str
    """Name of the bucket. See [bucket naming rules](https://docs.blyss.dev/docs/buckets#names)."""
    _public_uuid: Optional[str] = None

    def __init__(self, api: api.API, name: str, secret_seed: Optional[str] = None):
        """
        @private
        Initialize a client for a single, existing Blyss bucket.

        Args:
            api: A target API to send all underlying API calls to.
            name: The name of the bucket.
            secret_seed: An optional secret seed to initialize the client with.
                         A random one will be generated if not supplied.
        """
        self._basic_init(api, name, secret_seed)
        self._metadata = self._api._blocking_meta(self.name)
        self._lib = BlyssLib(
            json.dumps(self._metadata["pir_scheme"]), self._secret_seed
        )

    def _basic_init(self, api: api.API, name: str, secret_seed: Optional[str]):
        self.name: str = name
        # Internal attributes
        self._api = api
        if secret_seed:
            self._secret_seed = secret_seed
        else:
            self._secret_seed = seed.get_random_seed()

    def _check(self) -> bool:
        """Checks if the server has this client's public params.

        Args:
            uuid (str): The key to check.

        Returns:
            bool: Whether the server has the given UUID.
        """
        if self._public_uuid is None:
            raise RuntimeError("Bucket not initialized. Call setup() first.")
        return self._api._blocking_check(self._public_uuid)

    def _split_into_json_chunks(
        self, kv_pairs: dict[str, Optional[bytes]]
    ) -> list[dict[str, Optional[str]]]:
        _MAX_PAYLOAD = 5 * 2**20  # 5 MiB

        # 1. Bin keys by row index
        keys_by_index: dict[int, list[str]] = {}
        for k in kv_pairs.keys():
            i = self._lib.get_row(k)
            if i in keys_by_index:
                keys_by_index[i].append(k)
            else:
                keys_by_index[i] = [k]

        # 2. Prepare chunks of items, where each is a JSON-ready structure.
        # Each chunk is less than the maximum payload size, and guarantees
        # zero overlap of rows across chunks.
        kv_chunks: list[dict[str, Optional[str]]] = []
        current_chunk: dict[str, Optional[str]] = {}
        current_chunk_size = 0
        sorted_indices = sorted(keys_by_index.keys())
        for i in sorted_indices:
            keys = keys_by_index[i]
            # prepare all keys in this row
            row = {}
            row_size = 0
            for key in keys:
                vi = kv_pairs[key]
                if vi is None:
                    v = None
                else:
                    v = base64.b64encode(vi).decode("utf-8")
                row[key] = v
                row_size += int(
                    16 + len(key) + (len(v) if v is not None else 4)
                )  # 4 bytes for 'null'

            # if the new row doesn't fit into the current chunk, start a new one
            if current_chunk_size + row_size > _MAX_PAYLOAD:
                kv_chunks.append(current_chunk)
                current_chunk = row
                current_chunk_size = row_size
            else:
                current_chunk.update(row)
                current_chunk_size += row_size

        # add the last chunk
        if len(current_chunk) > 0:
            kv_chunks.append(current_chunk)

        return kv_chunks

    def _generate_query_stream(self, keys: list[str]) -> list[bytes]:
        assert self._public_uuid
        # generate encrypted queries
        queries: list[bytes] = [
            self._lib.generate_query(self._public_uuid, self._lib.get_row(k))
            for k in keys
        ]
        return queries

    def _decode_result(
        self, key: str, result_row: bytes, silence_errors: bool = True
    ) -> Optional[bytes]:
        try:
            decrypted_result = self._lib.decode_response(result_row)
            decompressed_result = bz2.decompress(decrypted_result)
            return self._lib.extract_result(key, decompressed_result)
        except:
            if not silence_errors:
                raise
            return None

    def setup(self):
        """Prepares this bucket client for private reads.

        This method will be called automatically by :method:`read`, but
        clients may call it explicitly prior to make subsequent
        `private_read` calls faster.

        Can upload significant amounts of data (1-10 MB).

        """
        public_params = self._lib.generate_keys_with_public_params()
        self._public_uuid = self._api._blocking_setup(self.name, public_params)
        assert self._check()

    def info(self) -> dict[Any, Any]:
        """Fetch this bucket's properties from the service, such as access permissions and PIR scheme parameters."""
        return self._api._blocking_meta(self.name)

    def rename(self, new_name: str):
        """Rename this bucket to new_name."""
        bucket_create_req = {
            "name": new_name,
        }
        self._api._blocking_modify(self.name, bucket_create_req)
        self.name = new_name

    def write(self, kv_pairs: dict[str, Optional[bytes]]):
        """Writes the supplied key-value pair(s) into the bucket.

        Args:
            kv_pairs: A dictionary of key-value pairs to write into the bucket.
                      Keys must be UTF8 strings, and values may be arbitrary bytes.
        """
        kv_json = {
            k: base64.b64encode(v).decode("utf-8") if v else None
            for k, v in kv_pairs.items()
        }
        self._api._blocking_write(self.name, kv_json)

    def delete_key(self, keys: str | list[str]):
        """Deletes key-value pairs from the bucket.

        Args:
            key: The key to delete.
        """
        if isinstance(keys, str):
            keys = [keys]

        # Writing None to a key is interpreted as a delete.
        delete_payload = {k: None for k in keys}
        self._api._blocking_write(self.name, delete_payload)

    def destroy_entire_bucket(self):
        """Destroys the entire bucket. This action is permanent and irreversible."""
        self._api._blocking_destroy(self.name)

    def clear_entire_bucket(self):
        """Deletes all keys in this bucket. This action is permanent and irreversible.

        Differs from destroy in that the bucket's metadata
        (e.g. permissions, PIR scheme parameters, and clients' setup data) are preserved.
        """
        self._api._blocking_clear(self.name)

    def private_read(self, keys: list[str]) -> list[Optional[bytes]]:
        """Privately reads the supplied key(s) from the bucket,
        and returns the corresponding value(s).

        Data will be accessed using fully homomorphic encryption, designed to
        make it impossible for any entity (including the Blyss service!) to
        determine which key(s) are being read.

        Args:
            keys: A key or list of keys to privately retrieve.
                  If a list of keys is supplied,
                  results will be returned in the same order.

        Returns:
            For each key, the value found for the key in the bucket,
            or None if the key was not found.
        """
        single_query = False
        if isinstance(keys, str):
            keys = [keys]
            single_query = True

        if not self._public_uuid or not self._check():
            self.setup()
            assert self._public_uuid

        queries = self._generate_query_stream(keys)
        rows_per_result = self._api._blocking_private_read(self.name, queries)
        results = [
            self._decode_result(key, result) if result else None
            for key, result in zip(keys, rows_per_result)
        ]

        if single_query:
            return results[0]

        return results

    def private_key_intersect(self, keys: list[str]) -> list[str]:
        """Privately intersects the given set of keys with the keys in this bucket,
        returning the keys that intersected. This is generally slower than a single
        private read, but much faster than making a private read for each key.

        Has the same privacy guarantees as private_read - zero information is leaked
        about keys being intersected.

        Requires that the bucket was created with key_storage_policy of "bloom" or "full".
        If the bucket cannot support private bloom filter lookups, an exception will be raised.

        Args:
            keys: A list of keys to privately intersect with this bucket.
        """
        bloom_filter = self._api._blocking_bloom(self.name)
        present_keys = list(filter(bloom_filter.lookup, keys))
        return present_keys


class AsyncBucket(Bucket):
    """Asyncio-compatible version of Bucket."""

    def __init__(self, api: api.API, name: str, secret_seed: Optional[str] = None):
        self._basic_init(api, name, secret_seed)

    async def async_init(self):
        """Python constructors can't be async, so instances of `AsyncBucket` must call this method after construction."""
        self._metadata = await self._api.meta(self.name)
        self._lib = BlyssLib(
            json.dumps(self._metadata["pir_scheme"]), self._secret_seed
        )

    async def _check(self) -> bool:
        if self._public_uuid is None:
            raise RuntimeError("Bucket not initialized. Call setup() first.")
        try:
            await self._api.check(self._public_uuid)
            return True
        except api.ApiException as e:
            if e.code == 404:
                return False
            else:
                raise e

    async def setup(self):
        public_params = self._lib.generate_keys_with_public_params()
        self._public_uuid = await self._api.setup(self.name, public_params)
        assert await self._check()

    async def info(self) -> dict[str, Any]:
        return await self._api.meta(self.name)

    async def rename(self, new_name: str):
        bucket_create_req = {
            "name": new_name,
        }
        await self._api.modify(self.name, bucket_create_req)
        self.name = new_name

    async def delete_key(self, keys: str | list[str]):
        keys = [keys] if isinstance(keys, str) else keys
        delete_payload: dict[str, Optional[str]] = {k: None for k in keys}
        await self._api.write(self.name, delete_payload)

    async def destroy_entire_bucket(self):
        await self._api.destroy(self.name)

    async def clear_entire_bucket(self):
        await self._api.clear(self.name)

    async def write(self, kv_pairs: dict[str, Optional[bytes]], CONCURRENCY=4):
        """
        Functionally equivalent to Bucket.write.

        Handles chunking and parallel submission of writes, up to CONCURRENCY.
        For maximum performance, call this function with as much data as possible.
        Data races are possible with parallel writes, but will never corrupt data.

        Args:
            CONCURRENCY: The number of concurrent server writes. Maximum is 8.
        """
        CONCURRENCY = min(CONCURRENCY, 8)

        # Split the key-value pairs into chunks not exceeding max payload size.
        # kv_chunks are JSON-ready, i.e. values are base64-encoded strings.
        kv_chunks = self._split_into_json_chunks(kv_pairs)
        # Make one write call per chunk, while respecting a max concurrency limit.
        sem = asyncio.Semaphore(CONCURRENCY)

        async def _paced_writer(chunk: dict[str, Optional[str]]):
            async with sem:
                await self._api.write(self.name, chunk)

        _tasks = [asyncio.create_task(_paced_writer(c)) for c in kv_chunks]
        await asyncio.gather(*_tasks)

    async def private_read(self, keys: list[str]) -> list[Optional[bytes]]:
        if not self._public_uuid or not await self._check():
            await self.setup()
            assert self._public_uuid

        multi_query = self._generate_query_stream(keys)

        rows_per_result = await self._api.async_private_read(self.name, multi_query)

        results = [
            self._decode_result(key, result) if result else None
            for key, result in zip(keys, rows_per_result)
        ]

        return results
