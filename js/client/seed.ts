import { isNode } from '../lib/util';

const SEED_BYTES = 32;
const SEED_STR_LEN = 44;

export function bytesToBase64(arr: Uint8Array): string {
  if (isNode()) {
    return Buffer.from(arr).toString('base64');
  } else {
    const output = [];

    for (let i = 0; i < arr.length; i++) {
      output.push(String.fromCharCode(arr[i]));
    }

    return btoa(output.join(''));
  }
}

export function base64ToBytes(inp: string): Uint8Array {
  if (isNode()) {
    return Buffer.from(inp, 'base64');
  } else {
    return Uint8Array.from(atob(inp), c => c.charCodeAt(0));
  }
}

export function seedFromString(seedStr: string): Uint8Array {
  if (seedStr.length !== SEED_STR_LEN)
    throw new Error('Incorrect seed length.');
  const seed = base64ToBytes(seedStr);
  if (seed.length !== SEED_BYTES) throw new Error('Incorrect seed length.');
  return seed;
}

export function stringFromSeed(seed: Uint8Array): string {
  if (seed.length !== SEED_BYTES) throw new Error('Incorrect seed length.');
  const seedStr = bytesToBase64(seed);
  if (seedStr.length !== SEED_STR_LEN)
    throw new Error('Incorrect seed length.');
  return seedStr;
}

export function getRandomSeed(): string {
  const seed = new Uint8Array(SEED_BYTES);
  let cryptoRef;
  if (typeof crypto === 'undefined') {
    cryptoRef = require('node:crypto');
  } else {
    cryptoRef = crypto;
  }
  cryptoRef.getRandomValues(seed);
  return stringFromSeed(seed);
}

export function getInsecureFixedSeed(): string {
  const seed = new Uint8Array(SEED_BYTES);
  return stringFromSeed(seed);
}
