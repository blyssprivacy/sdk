"""Bucket

Abstracts functionality on an existing bucket.
"""

from typing import Optional, Any, Union, Iterator

from . import api, serializer, seed
from .blyss_lib import BlyssLib

import json
import bz2
import time


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

    def _private_read(self, keys: list[str]) -> list[tuple[bytes, Optional[dict[Any, Any]]]]:
        """Performs the underlying private retrieval.

        Args:
            keys (str): A list of keys to retrieve.

        Returns:
            tuple[bytes, Optional[dict]]: Returns a tuple of (value, optional_metadata).
        """
        if not self.public_uuid or not self._check(self.public_uuid):
            self.setup()
            assert self.public_uuid

        # generate encrypted queries
        queries: list[bytes] = [
            self.lib.generate_query(self.public_uuid, self.lib.get_row(k)) 
            for k in keys
        ]
        # interleave the queries with their lengths (uint64_t)
        query_lengths = [len(q).to_bytes(8, "little") for q in queries]
        lengths_and_queries = [x for lq in zip(query_lengths, queries) for x in lq]
        # prepend the total number of queries (uint64_t)
        lengths_and_queries.insert(0, len(queries).to_bytes(8, "little"))
        # serialize the queries
        multi_query = b"".join(lengths_and_queries)
        
        start = time.perf_counter()
        multi_result = self.api.private_read(self.name, multi_query)
        self.exfil = time.perf_counter() - start

        retrievals = [] 
        for key, result in zip(keys, _chunk_parser(multi_result)):
            decrypted_result = self.lib.decode_response(result)
            decompressed_result = bz2.decompress(decrypted_result)
            extracted_result = self.lib.extract_result(key, decompressed_result)
            output = serializer.deserialize(extracted_result)
            retrievals.append(output)

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
