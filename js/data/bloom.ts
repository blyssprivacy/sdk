import { mergeUint8Arrays } from './serializer';

export type BloomFilter = {
  k: number;
  bits: number;
  data: Uint8Array;
};

export function bloomFilterFromBytes(rawData: Uint8Array): BloomFilter {
  const dv = new DataView(rawData.buffer);
  const k = dv.getUint32(0, true);
  const bits = dv.getUint32(4, true);
  const data = rawData.slice(8);
  return { k, bits, data };
}

function topBEBits(data: Uint8Array, bits: number): number {
  let num = 0;
  for (let i = 0; i < bits; i++) {
    const bit = data[Math.floor(i / 8)] & (1 << (7 - (i % 8)));
    if (bit != 0) {
      num += 1 << (bits - 1 - i);
    }
  }
  return num;
}

function toLEBytesUint32(num: number): Uint8Array {
  const ab = new ArrayBuffer(4);
  const dv = new DataView(ab);
  dv.setUint32(0, num, true);
  return new Uint8Array(ab);
}

async function bloomHash(
  bloomFilter: BloomFilter,
  key: string,
  hashIdx: number
): Promise<number> {
  const dataToHash = mergeUint8Arrays(
    toLEBytesUint32(hashIdx),
    new TextEncoder().encode(key)
  );
  const hashVal = new Uint8Array(
    await crypto.subtle.digest('SHA-1', dataToHash)
  );
  const num = topBEBits(hashVal, bloomFilter.bits);
  return num;
}

function checkBit(data: Uint8Array, idx: number): boolean {
  return (data[Math.floor(idx / 8)] & (1 << (7 - (idx % 8)))) != 0;
}

function setBit(data: Uint8Array, idx: number) {
  data[Math.floor(idx / 8)] |= 1 << (7 - (idx % 8));
}

export async function bloomLookup(
  bloomFilter: BloomFilter,
  key: string
): Promise<boolean> {
  for (let i = 0; i < bloomFilter.k; i++) {
    const idx = await bloomHash(bloomFilter, key, i);
    const check = checkBit(bloomFilter.data, idx);
    if (!check) return false;
  }

  return true;
}

export async function bloomWrite(
  bloomFilter: BloomFilter,
  key: string
): Promise<void> {
  for (let i = 0; i < bloomFilter.k; i++) {
    const idx = await bloomHash(bloomFilter, key, i);
    setBit(bloomFilter.data, idx);
  }
}

export function bloomInit(k: number, bits: number) {
  return {
    k,
    bits,
    data: new Uint8Array(1 << (bits - 3))
  };
}
