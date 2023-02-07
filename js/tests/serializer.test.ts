import {
  serialize,
  deserialize,
  deserializeChunks,
  serializeChunks
} from '../data/serializer';

describe('serialization', () => {
  it.each([undefined, () => 'foo' /*{ a: "foo", b: { c: () => "foo" } }*/])(
    `should fail for: %s`,
    val => {
      expect(() => serialize(val)).toThrow();
    }
  );
});

describe('serialization/deserialization routines', () => {
  it.each([
    [0, 0, 0],
    'foo',
    { a: 'foo' },
    { a: 'foo', b: { c: 'bar', d: ['baz'] } },
    0,
    null
  ])(`should be inverses for: %s`, val => {
    expect(deserialize(serialize(val)).data).toEqual(val);
  });
});

describe('chunk serialization/deserialization routines', () => {
  it.each([
    [
      [
        new Uint8Array([0, 1, 0]),
        new Uint8Array([77, 12, 10]),
        new Uint8Array([0]),
        new Uint8Array([190, 1, 4, 6, 1])
      ]
    ],
    [[new Uint8Array([0, 1, 0])]],
    [[new Uint8Array([1])]],
    [[new Uint8Array([])]],
    [[]]
  ])(`should be inverses for: %s`, val => {
    expect(deserializeChunks(serializeChunks(val))).toEqual(val);
  });
});
