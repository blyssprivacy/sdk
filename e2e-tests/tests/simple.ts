import type { Bucket } from '@blyss/sdk';
const blyss = require('@blyss/sdk/node');

export default async function main() {
  const bucket: Bucket = await blyss.Bucket.initializeLocal(
    'http://localhost:8008'
  );

  await bucket.write({
    Ohio: 'Columbus',
    California: 'Sacramento'
  });

  let capital = await bucket.privateRead('Ohio');
  if (capital !== 'Columbus') {
    throw 'Incorrect result.';
  }

  capital = await bucket.privateRead('California');
  if (capital !== 'Sacramento') {
    throw 'Incorrect result.';
  }
}
