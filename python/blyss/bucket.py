"""Bucket

Abstracts functionality on an existing bucket.
"""

from typing import Optional, Any, Union, Iterator

from . import api, serializer, seed
from .blyss_lib import BlyssLib

import json
import base64
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
    """A class representing a client to a single Blyss bucket."""

    def __init__(self, api: api.API, name: str, secret_seed: Optional[str] = None):
        """Initialize a client for a single, existing Blyss bucket.

        Args:
            api (api.API): A target API to send all underlying API calls to.
            name (str): The name of the bucket.
            secret_seed (Optional[str], optional): An optional secret seed to
            initialize the client with. A random one will be generated if not
            supplied. Defaults to None.
        """
        self.api = api
        self.name = name
        self.metadata = self.api.meta(self.name)
        self.secret_seed = seed.get_random_seed()
        if secret_seed:
            self.secret_seed = secret_seed
        self.lib = BlyssLib(json.dumps(self.metadata["pir_scheme"]), self.secret_seed)
        self.public_uuid: Optional[str] = None
        self.exfil: Any = None

    def _check(self, uuid: str) -> bool:
        """Checks if the server has the given UUID.

        Args:
            uuid (str): The key to check.

        Returns:
            bool: Whether the server has the given UUID.
        """
        try:
            self.api.check(uuid)
            return True
        except api.ApiException as e:
            if e.code == 404:
                return False
            else:
                raise e

    async def _async_check(self, uuid: str) -> bool:
        try:
            await self.api.async_check(uuid)
            return True
        except api.ApiException as e:
            if e.code == 404:
                return False
            else:
                raise e

    def _split_into_chunks(
        self, kv_pairs: dict[str, bytes]
    ) -> list[list[dict[str, str]]]:
        _MAX_PAYLOAD = 5 * 2**20  # 5 MiB

        # 1. Bin keys by row index
        keys_by_index: dict[int, list[str]] = {}
        for k in kv_pairs.keys():
            i = self.lib.get_row(k)
            if i in keys_by_index:
                keys_by_index[i].append(k)
            else:
                keys_by_index[i] = [k]

        # 2. Prepare chunks of items, where each is a JSON-ready structure.
        # Each chunk is less than the maximum payload size, and guarantees
        # zero overlap of rows across chunks.
        kv_chunks: list[list[dict[str, str]]] = []
        current_chunk: list[dict[str, str]] = []
        current_chunk_size = 0
        sorted_indices = sorted(keys_by_index.keys())
        for i in sorted_indices:
            keys = keys_by_index[i]
            # prepare all keys in this row
            row = []
            row_size = 0
            for key in keys:
                value = kv_pairs[key]
                value_str = base64.b64encode(value).decode("utf-8")
                fmt = {
                    "key": key,
                    "value": value_str,
                    "content-type": "application/octet-stream",
                }
                row.append(fmt)
                row_size += int(72 + len(key) + len(value_str))

            # if the new row doesn't fit into the current chunk, start a new one
            if current_chunk_size + row_size > _MAX_PAYLOAD:
                kv_chunks.append(current_chunk)
                current_chunk = row
                current_chunk_size = row_size
            else:
                current_chunk.extend(row)
                current_chunk_size += row_size

        # add the last chunk
        if len(current_chunk) > 0:
            kv_chunks.append(current_chunk)

        return kv_chunks

    def _generate_query_stream(self, keys: list[str]) -> bytes:
        # generate encrypted queries
        queries: list[bytes] = [
            self.lib.generate_query(self.public_uuid, self.lib.get_row(k)) for k in keys
        ]
        # interleave the queries with their lengths (uint64_t)
        query_lengths = [len(q).to_bytes(8, "little") for q in queries]
        lengths_and_queries = [x for lq in zip(query_lengths, queries) for x in lq]
        # prepend the total number of queries (uint64_t)
        lengths_and_queries.insert(0, len(queries).to_bytes(8, "little"))
        # serialize the queries
        multi_query = b"".join(lengths_and_queries)
        return multi_query

    def _unpack_query_result(
        self, keys: list[str], raw_result: bytes, parse_metadata: bool = True
    ) -> list[bytes]:
        retrievals = []
        for key, result in zip(keys, _chunk_parser(raw_result)):
            decrypted_result = self.lib.decode_response(result)
            decompressed_result = bz2.decompress(decrypted_result)
            extracted_result = self.lib.extract_result(key, decompressed_result)
            if parse_metadata:
                output = serializer.deserialize(extracted_result)
            else:
                output = extracted_result
            retrievals.append(output)
        return retrievals

    def _private_read(self, keys: list[str]) -> list[bytes]:
        """Performs the underlying private retrieval.

        Args:
            keys (str): A list of keys to retrieve.

        Returns:
            tuple[bytes, Optional[dict]]: Returns a tuple of (value, optional_metadata).
        """
        if not self.public_uuid or not self._check(self.public_uuid):
            self.setup()
            assert self.public_uuid

        multi_query = self._generate_query_stream(keys)

        start = time.perf_counter()
        multi_result = self.api.private_read(self.name, multi_query)
        self.exfil = time.perf_counter() - start

        retrievals = self._unpack_query_result(keys, multi_result)

        return retrievals

    def setup(self, uuid: Optional[str] = None):
        """Prepares this bucket client for private reads.

        This method will be called automatically by :method:`read`, but
        clients may call it explicitly prior to make subsequent
        :method:`read` calls faster.

        Can upload significant amounts of data (1-10 MB).
        """
        if uuid is not None and self._check(uuid):
            self.lib.generate_keys()
            self.public_uuid = uuid
        else:
            public_params = self.lib.generate_keys_with_public_params()
            setup_resp = self.api.setup(self.name, bytes(public_params))
            self.public_uuid = setup_resp["uuid"]

    def info(self) -> dict[Any, Any]:
        """Gets info on this bucket from the service."""
        return self.api.meta(self.name)

    def list_keys(self) -> dict[str, Any]:
        """Gets info on all keys in this bucket."""
        return self.api.list_keys(self.name)

    def write(self, kv_pairs: dict[str, Union[tuple[Any, Optional[Any]], Any]]):
        """Writes the supplied key-value pair(s) into the bucket.

        To supply metadata for a key, set the value in
        the dict to a tuple of (value_to_write, metadata).

        Args:
            kv_pairs (dict[str, Union[tuple[Any, Optional[Any]], Any]]):
                A dictionary containing the key-value pairs to write.
                Keys must be strings, and values may be any JSON-serializable value,
                bytes, or a tuple (see above).
        """
        concatenated_kv_items = b""
        for key, value in kv_pairs.items():
            if isinstance(value, tuple):
                value, metadata = value
            else:
                _ = value
                metadata = None

            serialized_value = serializer.serialize(value, metadata)
            concatenated_kv_items += serializer.wrap_key_val(
                key.encode("utf-8"), serialized_value
            )
        # single call to API endpoint
        self.api.write(self.name, concatenated_kv_items)

    def delete_key(self, key: str):
        """Deletes the supplied key from the bucket.

        Args:
            key (str): The key to delete.
        """
        self.api.delete_key(self.name, key)

    def destroy_entire_bucket(self):
        """Destroys the entire bucket. This action is permanent and irreversible."""
        self.api.destroy(self.name)

    def private_read(self, keys: Union[str, list[str]]) -> Union[bytes, list[bytes]]:
        """Privately reads the supplied key from the bucket,
        returning the value corresponding to the key.

        No entity, including the Blyss service, should be able to
        determine which key(s) this method was called for.

        Args:
            keys (str): A key or list of keys to privately read.
                        If a list of keys is supplied,
                        results will be returned in the same order.

        Returns:
            bytes: The value found for the key in the bucket,
                   or None if the key was not found.
        """
        single_query = False
        if isinstance(keys, str):
            keys = [keys]
            single_query = True

        results = [r[0] for r in self._private_read(keys)]
        if single_query:
            return results[0]

        return results

    def private_key_intersect(self, keys: list[str]) -> list[str]:
        """Privately intersects the given set of keys with the keys in this bucket,
        returning the keys that intersected. This is generally slower than a single
        private read.

        No entity, including the Blyss service, should be able to determine which
        keys this method was called for.

        Args:
            keys (list[str]): The keys to _privately_ intersect the value of.
        """
        bloom_filter = self.api.bloom(self.name)
        present_keys = list(filter(bloom_filter.lookup, keys))
        return present_keys


class AsyncBucket(Bucket):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

    async def write(self, kv_pairs: dict[str, bytes], MAX_CONCURRENCY=8):
        # Split the key-value pairs into chunks not exceeding max payload size.
        kv_chunks = self._split_into_chunks(kv_pairs)
        # Make one write call per chunk, while respecting a max concurrency limit.
        sem = asyncio.Semaphore(MAX_CONCURRENCY)

        async def _paced_writer(chunk):
            async with sem:
                await self.api.async_write(self.name, json.dumps(chunk))

        _tasks = [asyncio.create_task(_paced_writer(c)) for c in kv_chunks]
        await asyncio.gather(*_tasks)

    async def private_read(self, keys: list[str]) -> list[bytes]:
        if not self.public_uuid or not await self._async_check(self.public_uuid):
            self.setup()
            assert self.public_uuid

        multi_query = self._generate_query_stream(keys)

        start = time.perf_counter()
        multi_result = await self.api.async_private_read(self.name, multi_query)
        self.exfil = time.perf_counter() - start

        retrievals = self._unpack_query_result(keys, multi_result, parse_metadata=False)

        return retrievals
