"""Bucket

Abstracts functionality on an existing bucket.
"""

from typing import Optional, Any, Union, Iterator

from . import api, seed
from .blyss_lib import BlyssLib

import json
import bz2
import time
import asyncio


def _chunk_parser(raw_data: bytes) -> Iterator[bytes]:
    """
    Parse a bytestream containing an arbitrary number of length-prefixed chunks.

    """
    data = memoryview(raw_data)
    i = 0
    num_chunks = int.from_bytes(data[:8], "little", signed=False)
    i += 8
    for _ in range(num_chunks):
        chunk_len = int.from_bytes(data[i : i + 8], "little", signed=False)
        i += 8
        chunk_data = bytes(data[i : i + chunk_len])
        i += chunk_len
        yield chunk_data


class Bucket:
    """Interface to a single Blyss bucket."""

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
        self.name: str = name
        """Name of the bucket. See [bucket naming rules](https://docs.blyss.dev/docs/buckets#names)."""

        # Internal attributes
        self._api = api
        self._metadata = self._api.meta(self.name)
        if secret_seed:
            self._secret_seed = secret_seed
        else:
            self._secret_seed = seed.get_random_seed()
        self._lib = BlyssLib(
            json.dumps(self._metadata["pir_scheme"]), self._secret_seed
        )
        self._public_uuid: Optional[str] = None
        self._exfil: Any = None  # used for benchmarking

    def _check(self) -> bool:
        """Checks if the server has this client's public params.

        Args:
            uuid (str): The key to check.

        Returns:
            bool: Whether the server has the given UUID.
        """
        if self._public_uuid is None:
            raise RuntimeError("Bucket not initialized. Call setup() first.")
        try:
            self._api.check(self._public_uuid)
            return True
        except api.ApiException as e:
            if e.code == 404:
                return False
            else:
                raise e

    def _split_into_chunks(self, kv_pairs: dict[str, bytes]) -> list[dict[str, bytes]]:
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
        kv_chunks: list[dict[str, bytes]] = []
        current_chunk: dict[str, bytes] = {}
        current_chunk_size = 0
        sorted_indices = sorted(keys_by_index.keys())
        for i in sorted_indices:
            keys = keys_by_index[i]
            # prepare all keys in this row
            row = {}
            row_size = 0
            for key in keys:
                v = kv_pairs[key]
                row[key] = v
                row_size += int(16 + len(key) + len(v) * 4 / 3)

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
        # interleave the queries with their lengths (uint64_t)
        query_lengths = [len(q).to_bytes(8, "little") for q in queries]
        lengths_and_queries = [x for lq in zip(query_lengths, queries) for x in lq]
        # prepend the total number of queries (uint64_t)
        lengths_and_queries.insert(0, len(queries).to_bytes(8, "little"))
        # serialize the queries
        multi_query = b"".join(lengths_and_queries)
        return multi_query

    def _decode_result(self, key: str, result_row: bytes) -> Optional[bytes]:
        try:
            decrypted_result = self._lib.decode_response(result_row)
            decompressed_result = bz2.decompress(decrypted_result)
            return self._lib.extract_result(key, decompressed_result)
        except:
            return None

    def _unpack_query_result(
        self, keys: list[str], raw_result: bytes, ignore_errors=False
    ) -> list[Optional[bytes]]:
        retrievals = []
        for key, result in zip(keys, _chunk_parser(raw_result)):
            if len(result) == 0:
                # error in processing this query
                if ignore_errors:
                    extracted_result = None
                else:
                    raise RuntimeError(f"Failed to process query for key {key}.")
            else:
                extracted_result = self._decode_result(key, result)
            retrievals.append(extracted_result)
        return retrievals

    def _private_read(self, keys: list[str]) -> list[Optional[bytes]]:
        """Performs the underlying private retrieval.

        Args:
            keys (str): A list of keys to retrieve.

        Returns:
            a list of values (bytes) corresponding to keys. None for keys not found.
        """
        if not self._public_uuid or not self._check():
            self.setup()
            assert self._public_uuid

        queries = self._generate_query_stream(keys)

        start = time.perf_counter()
        rows_per_result = self._api.private_read(self.name, queries)
        self._exfil = time.perf_counter() - start

        results = [
            self._decode_result(key, result) if result else None
            for key, result in zip(keys, rows_per_result)
        ]

        return results

    def setup(self):
        """Prepares this bucket client for private reads.

        This method will be called automatically by :method:`read`, but
        clients may call it explicitly prior to make subsequent
        `private_read` calls faster.

        Can upload significant amounts of data (1-10 MB).

        """
        public_params = self._lib.generate_keys_with_public_params()
        self._public_uuid = self._api.setup(self.name, bytes(public_params))
        assert self._check()

    def info(self) -> dict[Any, Any]:
        """Fetch this bucket's properties from the service, such as access permissions and PIR scheme parameters."""
        return self._api.meta(self.name)

    def list_keys(self) -> list[str]:
        """List all key strings in this bucket. Only available if bucket was created with keyStoragePolicy="full"."""
        return self._api.list_keys(self.name)

    def rename(self, new_name: str):
        """Rename this bucket to new_name."""
        bucket_create_req = {
            "name": new_name,
        }
        r = self._api.modify(self.name, bucket_create_req)
        print(r)
        self.name = new_name

    def write(self, kv_pairs: dict[str, bytes]):
        """Writes the supplied key-value pair(s) into the bucket.

        Args:
            kv_pairs: A dictionary of key-value pairs to write into the bucket.
                      Keys must be UTF8 strings, and values may be arbitrary bytes.
        """
        self._api.write(self.name, kv_pairs)  # type: ignore
        # bytes is a valid subset of Optional[bytes], despite mypy's complaints

    def delete_key(self, keys: str | list[str]):
        """Deletes key-value pairs from the bucket.

        Args:
            key: The key to delete.
        """
        if isinstance(keys, str):
            keys = [keys]

        delete_payload = {k: None for k in keys}
        self._api.write(self.name, delete_payload)  # type: ignore

    def destroy_entire_bucket(self):
        """Destroys the entire bucket. This action is permanent and irreversible."""
        self._api.destroy(self.name)

    def clear_entire_bucket(self):
        """Deletes all keys in this bucket. This action is permanent and irreversible.

        Differs from destroy in that the bucket's metadata
        (e.g. permissions, PIR scheme parameters, and clients' setup data) are preserved.
        """
        self._api.clear(self.name)

    def private_read(
        self, keys: Union[str, list[str]]
    ) -> Union[Optional[bytes], list[Optional[bytes]]]:
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

        results = self._private_read(keys)
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
        bloom_filter = self._api.bloom(self.name)
        present_keys = list(filter(bloom_filter.lookup, keys))
        return present_keys


class AsyncBucket(Bucket):
    """Asyncio-compatible version of Bucket."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

    async def _check(self) -> bool:
        if self._public_uuid is None:
            raise RuntimeError("Bucket not initialized. Call setup() first.")
        try:
            await self._api.async_check(self._public_uuid)
            return True
        except api.ApiException as e:
            if e.code == 404:
                return False
            else:
                raise e

    async def setup(self):
        public_params = self._lib.generate_keys_with_public_params()
        self._public_uuid = await self._api.async_setup(self.name, bytes(public_params))
        assert await self._check()

    async def write(self, kv_pairs: dict[str, bytes], CONCURRENCY=4):
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
        kv_chunks = self._split_into_chunks(kv_pairs)
        # Make one write call per chunk, while respecting a max concurrency limit.
        sem = asyncio.Semaphore(CONCURRENCY)

        async def _paced_writer(chunk: dict[str, bytes]):
            async with sem:
                await self._api.async_write(self.name, chunk)  # type: ignore
                # bytes is a valid subset of Optional[bytes], despite mypy's complaints

        _tasks = [asyncio.create_task(_paced_writer(c)) for c in kv_chunks]
        await asyncio.gather(*_tasks)

    async def private_read(self, keys: list[str]) -> list[Optional[bytes]]:
        if not self._public_uuid or not await self._check():
            await self.setup()
            assert self._public_uuid

        multi_query = self._generate_query_stream(keys)

        start = time.perf_counter()
        rows_per_result = await self._api.async_private_read(self.name, multi_query)
        self._exfil = time.perf_counter() - start

        results = [
            self._decode_result(key, result) if result else None
            for key, result in zip(keys, rows_per_result)
        ]

        return results
