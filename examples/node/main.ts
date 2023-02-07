import { Client } from '@blyss/client-sdk';

async function main() {
  const client = new Client('<YOUR API KEY HERE>');

  // Create the bucket
  const bucketName = 'state-capitals';
  if (!(await client.exists(bucketName))) {
    console.log('creating...');
    await client.create(bucketName);
  }

  // Connect to your bucket
  const bucket = await client.connect(bucketName);

  // Write some data to it
  bucket.write({
    California: 'Sacramento',
    Ohio: 'Columbus',
    'New York': 'Albany'
  });

  // This is a completely *private* query:
  // the server *cannot* learn that you looked up "California"!
  const capital = await bucket.privateRead('California');
  console.log(`Got capital: ${capital}`);

  // This is a completely *private* intersection operation:
  // the server *cannot* learn that the set was ['Wyoming', 'California', 'Ohio']!
  const setToTest = ['Wyoming', 'California', 'Ohio'];
  const intersection = await bucket.privateIntersect(setToTest);
  console.log(
    'Intersection of',
    setToTest,
    'and bucket yielded:',
    intersection
  );
}

main();
