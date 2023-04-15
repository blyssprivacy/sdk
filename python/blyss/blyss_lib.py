"""BlyssLib

INTERNAL

This Python module is the *only* code that interfaces with
the compiled Rust code.
"""


from typing import Any, Optional
from . import blyss, seed  # type: ignore

# NB:   There are many "type: ignore"s on purpose. Type information
#       for the maturin output is difficult to include nicely, so we just wrap it here.


class BlyssLib:
    """
    A class wrapping a bytes-for-bytes cryptographic interface for the Blyss service.
    """

    def generate_keys(self):
        """Generates the clients keys. Skips public parameter generation.

        Args:
            generate_public_parameters (bool): Whether the client should generate
            additional public parameters.
        """

        blyss.generate_keys(  # type: ignore
            self.inner_client,
            seed.seed_from_string(self.secret_seed),
            False,
        )

    def generate_keys_with_public_params(self) -> bytes:
        """Generates the clients keys, including public parameters.

        This can take a long time, so prefer `generate_keys` when possible.

        Returns:
            bytes: The generated public parameters, if requested.
        """

        return blyss.generate_keys(  # type: ignore
            self.inner_client,
            seed.seed_from_string(self.secret_seed),
            True,
        )

    def get_row(self, key: str) -> int:
        """Gets the target row in the database for a given key.

        Args:
            key (str): The key to find the row of.

        Returns:
            int: The row corresponding to the given key.
        """
        return blyss.get_row(self.inner_client, key)  # type: ignore

    def generate_query(self, uuid: str, row_idx: int) -> bytes:
        """Generates a query for the given row index.

        Args:
            uuid (str): The UUID of public parameters that have already been uploaded to the server.
            row_idx (int): The index of the target row of the query.

        Returns:
            bytes: The raw bytes the client should send to the server as its query.
        """
        return bytes(blyss.generate_query(self.inner_client, uuid, row_idx))  # type: ignore

    def decode_response(self, response: bytes) -> bytes:
        """Decodes the PIR response to plaintext, using the client's secrets.

        Args:
            response (bytes): The raw PIR response from the server.

        Returns:
            bytes: The plaintext data in the response.
        """
        return bytes(blyss.decode_response(self.inner_client, response))  # type: ignore

    def extract_result(self, key: str, data: bytes) -> Optional[bytes]:
        """Extracts the value for a given key, given the plaintext data from a response.

        Args:
            key (str): The key the client is looking up.
            data (bytes): The plaintext data from the PIR response.

        Returns:
            bytes: The plaintext data corresponding to the given key.
        """
        r = blyss.extract_result(self.inner_client, key, data)
        if r is None:
            return None
        else:
            return bytes(r)

    def __init__(self, params: str, secret_seed: str):
        """Initializes a new BlyssLib instance.

        Args:
            params (str): The set of JSON parameters for the underlying PIR scheme.
            secret_seed (str): A base64-encoded secret seed that is used to derive all client secrets.
        """
        self.inner_client: Any = blyss.initialize_client(params)  # type: ignore
        self.secret_seed = secret_seed
