import requests
from requests.adapters import HTTPAdapter
import io
import gzip

COMPRESSION_SCHEMES = [
    "http://",
    "https://",
]


class CompressionAdapter(HTTPAdapter):

    """Adapter used with `requests` library for sending compressed data."""

    CONTENT_LENGTH = "Content-Length"

    def add_headers(self, request, **kwargs):
        """Tell the server that we support compression."""
        super(CompressionAdapter, self).add_headers(request, **kwargs)

        body = request.body
        if isinstance(body, bytes):
            content_length = len(body)
        else:
            content_length = body.seek(0, 2)
            body.seek(0, 0)

        headers = {
            "Content-Encoding": "gzip",
            "Accept-Encoding": "gzip",
            "Content-Length": content_length,
        }
        request.headers.update(headers)

    def send(self, request, stream=False, **kwargs):
        """Compress data before sending."""
        if stream:
            # Having a file-like object, therefore we need to stream the
            # content into a new one through the compressor.
            compressed_body = io.BytesIO()
            compressed_body.name = request.url
            compressor = gzip.open(compressed_body, mode="wb")
            # Read, write and compress the content at the same time.
            compressor.write(request.body.read())
            compressor.flush()
            compressor.close()
            # Make available the new compressed file-like object as the new
            # request body.
            compressed_body.seek(0, 0)  # make it readable
            request.body = compressed_body
        else:
            # We're dealing with a plain bytes content, so compress it.
            request.body = gzip.compress(request.body)

        return super(CompressionAdapter, self).send(request, stream=stream, **kwargs)


def get_session():
    """Get a requests session supporting compression."""
    # Prepare the adapter and the session.
    compression_adapter = CompressionAdapter()
    session = requests.Session()

    # Mount the adapter to all affected schemes.
    for scheme in COMPRESSION_SCHEMES:
        session.mount(scheme, compression_adapter)

    # Use this sessions for CRUD-ing.
    return session
