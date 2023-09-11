import type { Bucket } from '@blyss/sdk';
const blyss = require('@blyss/sdk/node');

export default async function main(port: string) {
  const bucket: Bucket = await blyss.Bucket.initializeLocal(
    'http://localhost:' + port
  );

  console.log(bucket.metadata);

  // buckets are bytes-in/bytes-out. SDK write() will automatically serialize as UTF-8.
  await bucket.write({
    Ohio: 'Columbus',
    California: 'Sacramento'
  });

  // but reads are always bytes-out, and must be decoded.
  let capital = new TextDecoder().decode(await bucket.privateRead('Ohio'));
  if (capital !== 'Columbus') {
    throw 'Incorrect result.';
  }

  // capital = await bucket.privateRead('California');
  capital = new TextDecoder().decode(await bucket.privateRead('California'));
  if (capital !== 'Sacramento') {
    throw 'Incorrect result.';
  }
}
