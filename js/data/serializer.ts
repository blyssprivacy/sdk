import * as varint from './varint';

export function serializeChunks(chunks: Uint8Array[]): Uint8Array {
  let totalLen = 0;
  for (const chunk of chunks) {
    totalLen += chunk.length;
  }

  const out = new Uint8Array(totalLen + chunks.length * 8 + 8);
  const dv = new DataView(out.buffer);
  dv.setBigUint64(0, BigInt(chunks.length), true);
  let offs = 8;
  for (const chunk of chunks) {
    dv.setBigUint64(offs, BigInt(chunk.length), true);
    offs += 8;
    out.set(chunk, offs);
    offs += chunk.length;
  }

  return out;
}

export function deserializeChunks(serializedChunks: Uint8Array): Uint8Array[] {
  const dv = new DataView(serializedChunks.buffer);
  let offs = 0;
  const numChunks = Number(dv.getBigUint64(offs, true));
  offs += 8;

  const chunks = [];
  for (let i = 0; i < numChunks; i++) {
    const chunkLen = Number(dv.getBigUint64(offs, true));
    offs += 8;
    const chunk = serializedChunks.slice(offs, offs + chunkLen);
    offs += chunkLen;
    chunks.push(chunk);
  }

  return chunks;
}

export function mergeUint8Arrays(
  arr1: Uint8Array,
  arr2: Uint8Array
): Uint8Array {
  const mergedArray = new Uint8Array(arr1.length + arr2.length);
  mergedArray.set(arr1);
  mergedArray.set(arr2, arr1.length);
  return mergedArray;
}

/**
 * Safely serializes an object into bytes.
 *
 * @param obj - Object to serialize.
 */
export function serialize(obj: any): Uint8Array {
  if (obj instanceof ArrayBuffer || obj instanceof Uint8Array) {
    return obj instanceof ArrayBuffer ? new Uint8Array(obj) : obj;
  }
  const encoder = new TextEncoder();
  const objJson = JSON.stringify(obj);
  if (objJson === undefined)
    throw new Error('Object does not properly serialize');
  return encoder.encode(objJson);
}

/**
 * Safely deserializes an object from input bytes.
 * If the input bytes are valid JSON, the object will be deserialized as JSON.
 * Otherwise, the input bytes will be returned as-is (Uint8Array).
 *
 * @param data - Bytes to deserialize.
 */
export function deserialize(data: Uint8Array): any {
  try {
    const decoder = new TextDecoder();
    const obj = JSON.parse(decoder.decode(data));
    return obj;
  } catch (e) {
    return data;
  }
}

/** Concatenate the input Uint8Arrays. */
export function concatBytes(arrays: Uint8Array[]): Uint8Array {
  let totalLen = 0;
  for (const arr of arrays) {
    totalLen += arr.length;
  }

  const output = new Uint8Array(totalLen);
  let idx = 0;
  for (const arr of arrays) {
    output.set(arr, idx);
    idx += arr.length;
  }

  return output;
}

/** Wraps a key and value into a single bytes sequence. */
export function wrapKeyValue(key: Uint8Array, value: Uint8Array): Uint8Array {
  const keyLenVarint = varint.encode(key.length);
  const valueLenVarint = varint.encode(value.length);
  return concatBytes([keyLenVarint, key, valueLenVarint, value]);
}
