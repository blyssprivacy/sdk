import { encode, decode } from '../data/varint';

describe('encode/decode routines', () => {
  it.each([
    0,
    1,
    127,
    128,
    12345678,
    100,
    1000,
    1 << 32,
    1 << 50,
    (1 << 32) - 1
  ])(`should be inverses for: %i`, val => {
    expect(decode(encode(val)).value).toEqual(val);
  });
});
