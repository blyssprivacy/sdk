import blyss

api_key = "<YOUR API KEY HERE>"
client = blyss.Client(api_key)

# Create the bucket and fill it with some data
bucket_name = "state-capitals"
bucket = None
if not client.exists(bucket_name):
    client.create(bucket_name)

# Connect to your bucket
bucket = client.connect(bucket_name)

# Write some data to it
bucket.write(
    {
        "California": b"Sacramento",
        "Ohio": b"Columbus",
        "New York": b"Albany",
    }
)

# This is a completely *private* query:
# the server *cannot* learn that you looked up "California"!
print("Privately reading the capital of California...")
query_results = bucket.private_read(["California"])
capital = query_results[0].decode("utf-8")
print(f"Got '{capital}'!")
