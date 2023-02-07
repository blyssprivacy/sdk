// adapted from github.com/chrisdickinson/varint

const MSB = 0x80,
  REST = 0x7f,
  MSBALL = ~REST,
  INT = Math.pow(2, 31);

/**
 * Encodes a number as a varint.
 * @param num - the number to encode
 * @returns a `Uint8Array` encoding the number as a varint
 */
export function encode(num: number): Uint8Array {
  if (Number.MAX_SAFE_INTEGER && num > Number.MAX_SAFE_INTEGER) {
    throw new RangeError("Could not encode varint");
  }
  const out = [];
  let offset = 0;

  while (num >= INT) {
    out[offset++] = (num & 0xff) | MSB;
    num /= 128;
  }
  while (num & MSBALL) {
    out[offset++] = (num & 0xff) | MSB;
    num >>>= 7;
  }
  out[offset] = num | 0;

  return new Uint8Array(out);
}

/**
 * Decodes a varint.
 * @param buf - the buffer to decode
 * @returns `value`, the value of the varint,
 *     and `bytesProcessed`, the number of bytes consumed
 */
export function decode(buf: Uint8Array): {
  value: number;
  bytesProcessed: number;
} {
  let res = 0,
    shift = 0,
    counter = 0,
    b;
  const l = buf.length;

  do {
    if (counter >= l || shift > 49) {
      throw new RangeError("Could not decode varint");
    }
    b = buf[counter++];
    res += shift < 28 ? (b & REST) << shift : (b & REST) * Math.pow(2, shift);
    shift += 7;
  } while (b >= MSB);

  return {
    value: res,
    bytesProcessed: counter,
  };
}
