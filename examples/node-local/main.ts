import type { Bucket } from '@blyss/sdk';
const blyss = require('@blyss/sdk/node');
process.removeAllListeners('warning');

async function main() {
  const bucket: Bucket = await blyss.Bucket.initializeLocal(
    'http://localhost:8008'
  );

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
