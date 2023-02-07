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

function getObjectAsBytes(obj: any): Uint8Array {
  if (obj instanceof ArrayBuffer || obj instanceof Uint8Array) {
    return obj instanceof ArrayBuffer ? new Uint8Array(obj) : obj;
  }
  const encoder = new TextEncoder();
  const objJson = JSON.stringify(obj);
  if (objJson === undefined)
    throw new Error('Object does not properly serialize');
  return encoder.encode(objJson);
}

function getHeaderBytes(obj: any, metadata?: any): Uint8Array {
  if (!metadata && (obj instanceof ArrayBuffer || obj instanceof Uint8Array)) {
    return varint.encode(0);
  } else {
    const encoder = new TextEncoder();
    const headerData = { contentType: 'application/json', ...metadata };
    const header = JSON.stringify(headerData);
    const headerVarInt = varint.encode(header.length);
    return mergeUint8Arrays(headerVarInt, encoder.encode(header));
  }
}

/**
 * Safely serializes an object (and optional metadta) into bytes.
 *
 * @param obj - Object to serialize.
 */
export function serialize(obj: any, metadata?: any): Uint8Array {
  const headerBytes = getHeaderBytes(obj, metadata);
  const objAsBytes = getObjectAsBytes(obj);

  return mergeUint8Arrays(headerBytes, objAsBytes);
}

export interface DataWithMetadata {
  data: any;
  metadata?: any;
}

/**
 * Safely deserializes an object, and possibly any associated metadata, from the
 * input bytes.
 *
 * @param data - Bytes to deserialize.
 */
export function deserialize(data: Uint8Array): DataWithMetadata {
  const { value, bytesProcessed } = varint.decode(data);
  const headerLength = value;
  let i = bytesProcessed;
  if (headerLength === 0) {
    return { data: data.slice(i) };
  }

  const decoder = new TextDecoder();
  const headerBytes = data.slice(i, i + headerLength);
  i += headerLength;
  const header = JSON.parse(decoder.decode(headerBytes));

  const dataBytes = data.slice(i);

  let obj;
  if (header.contentType === 'application/json') {
    obj = JSON.parse(decoder.decode(dataBytes));
  } else {
    obj = dataBytes;
  }

  return {
    data: obj,
    metadata: header
  };
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
