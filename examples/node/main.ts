import type { Client } from '@blyss/sdk';
const blyss = require('@blyss/sdk/node');
const fs = require('fs');

process.removeAllListeners('warning');

async function main() {
  const client: Client = new blyss.Client('<YOUR API KEY HERE>');

  // Create the bucket
  const bucketName = 'global.wc-v1';
  if (!(await client.exists(bucketName))) {
    console.log('creating...');
    await client.create(bucketName, true, {
      keyStoragePolicy: 'none'
    });
  }

  // Connect to your bucket
  const bucket = await client.connect(bucketName);

  // Write some data to it
  await bucket.write({
    Ohio: 'Columbus',
    California: 'Sacramento',
    Washington: 'Olympia'
  });

  // This is a completely *private* query:
  // the server *cannot* learn that you looked up "California"!
  const capital = await bucket.privateRead('California');
  console.log(`Got capital: ${capital}`);
}

main();
