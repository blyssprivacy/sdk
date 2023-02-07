import blyss
import logging
import requests
import json

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
        "California": "Sacramento",
        "Ohio": "Columbus",
        "New York": "Albany",
    }
)

# This is a completely *private* query:
# the server *cannot* learn that you looked up "California"!
print("Privately reading the capital of California...")
capital = bucket.private_read("California")
print(f"Got '{capital}'!")

# This is a completely *private* intersection operation:
# the server *cannot* learn that the set was ['Wyoming', 'California', 'Ohio']!
set_to_test = ["Wyoming", "California", "Ohio"]
intersection = bucket.private_key_intersect(set_to_test)
print(f"Intersection of {set_to_test} and bucket yielded: {intersection}")
