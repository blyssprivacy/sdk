import { seedFromString, stringFromSeed } from '../client/seed';

describe('seedFromString/stringFromSeed routines', () => {
  it.each([
    'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=',
    '11opbOgHEQUCgBcXSeOv7wnYCEJFmycNY+HhuypZQJY='
  ])(`should be inverses for: %s`, val => {
    expect(stringFromSeed(seedFromString(val))).toEqual(val);
  });
});
