import blyss

api_key = "<YOUR API KEY HERE>"
client = blyss.Client(api_key, "https://alpha.api.blyss.dev")

# Create the bucket and fill it with some data
bucket_name = "state-capitals"
bucket = None
if not client.exists(bucket_name):
    client.create(bucket_name)

# Connect to your bucket
bucket = client.connect(bucket_name)

# Write some data (keys are strings, values are bytes)
bucket.write(
    {
        "California": "Sacramento".encode(),
        "Ohio": "Columbus".encode(),
        "New York": "Albany".encode(),
    }
)

# This is a completely *private* query:
# the server *cannot* learn that you looked up "California" or "Texas"!
print("Privately reading the capital of California...")
capitals = bucket.private_read(["California", "Texas"])

# when a requested key is not found, its value is None
capitals = [c.decode() if c else None for c in capitals]
print(f"Got '{capitals}'!")
