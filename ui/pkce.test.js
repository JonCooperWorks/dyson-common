/* Tests for the PKCE helpers. */
import { describe, expect, test } from 'vitest';
import { base64url, randomString, pkceChallenge } from './pkce.js';

const BASE64URL = /^[A-Za-z0-9_-]+$/;

describe('base64url', () => {
  test('encodes bytes without padding or +//', () => {
    const out = base64url(new Uint8Array([255, 255, 255]));
    expect(out).toMatch(BASE64URL);
    expect(out).not.toContain('=');
  });
});

describe('randomString', () => {
  test('is base64url and differs between calls', () => {
    const a = randomString();
    const b = randomString();
    expect(a).toMatch(BASE64URL);
    expect(a).not.toBe(b);
  });
});

describe('pkceChallenge', () => {
  test('S256 challenge is 43-char base64url (32-byte digest, unpadded)', async () => {
    const challenge = await pkceChallenge(randomString());
    expect(challenge).toMatch(BASE64URL);
    expect(challenge).toHaveLength(43);
  });

  test('is deterministic for a given verifier', async () => {
    const verifier = 'test-verifier';
    expect(await pkceChallenge(verifier)).toBe(await pkceChallenge(verifier));
  });
});
