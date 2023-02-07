const keyInput = document.getElementById('key-input');
const resultOutput = document.getElementById('result-output');
const submitButton = document.getElementById('submit');

// This will be our client to the bucket
let bucket;

// This function gets called only on the first query
async function setup() {
  const client = new window.blyss.Client('<YOUR API KEY HERE>');

  // Create the bucket
  const bucketName = 'state-capitals';
  if (!(await client.exists(bucketName))) {
    await client.create(bucketName);
  }

  // Connect to your bucket
  bucket = await client.connect(bucketName);

  // Write some data to it
  bucket.write({
    California: 'Sacramento',
    Ohio: 'Columbus',
    'New York': 'Albany'
  });
}

// This function performs the query
async function privateRetrieve() {
  submitButton.innerText = '...';
  submitButton.disabled = true;

  if (!bucket) await setup();

  // This is a completely *private* query:
  // the server *cannot* learn anything about 'keyToRetrieve'!
  const keyToRetrieve = keyInput.value;
  const result = await bucket.privateRead(keyToRetrieve);

  resultOutput.innerText = result;

  submitButton.innerText = '>';
  submitButton.disabled = false;
}

// Perform the query when we click the button
submitButton.onclick = privateRetrieve;
