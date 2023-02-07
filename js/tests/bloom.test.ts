import { bloomLookup, bloomWrite, bloomInit } from '../data/bloom';

describe('bloom filter write + lookup', () => {
  it.each([
    [10, 24, ['a', 'b', 'c']],
    [8, 18, ['x', 'y', 'z']]
  ])(`should work`, async (k, bits, vals) => {
    const filter = bloomInit(k, bits);
    for (const val of vals) {
      bloomWrite(filter, val);
    }
    for (const val of vals) {
      const got = await bloomLookup(filter, val);
      expect(got).toEqual(true);
    }
    for (const val of vals) {
      const got = await bloomLookup(filter, val + '############');
      expect(got).toEqual(false);
    }
  });
});
