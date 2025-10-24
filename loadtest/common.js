import encoding from 'k6/encoding';
import { TextEncoder } from 'https://raw.githubusercontent.com/inexorabletash/text-encoding/master/index.js';

const HMAC_KEY = __ENV.IMGFORGE_KEY || '';
const HMAC_SALT = __ENV.IMGFORGE_SALT || '';
const USE_UNSIGNED = __ENV.IMGFORGE_ALLOW_UNSIGNED === 'true';

export function hexToBytes(hexString) {
    const normalized = hexString.trim().replace(/^0x/, '');

    if (normalized.length === 0) {
        return new Uint8Array([]);
    }

    if (normalized.length % 2 !== 0) {
        throw new Error('IMGFORGE_KEY and IMGFORGE_SALT must be valid hex strings');
    }

    const bytes = new Uint8Array(normalized.length / 2);
    for (let i = 0; i < normalized.length; i += 2) {
        const byte = parseInt(normalized.slice(i, i + 2), 16);
        if (Number.isNaN(byte)) {
            throw new Error('IMGFORGE_KEY and IMGFORGE_SALT must be valid hex strings');
        }
        bytes[i / 2] = byte;
    }

    return bytes;
}

export async function generateSignature(path) {
    if (USE_UNSIGNED) {
        return 'unsafe';
    }

    const subtle = globalThis.crypto && globalThis.crypto.subtle;
    if (!subtle) {
        throw new Error('Web Crypto API is not available in this environment.');
    }

    const keyBytes = hexToBytes(HMAC_KEY);
    const saltBytes = hexToBytes(HMAC_SALT);
    const pathBytes = new TextEncoder().encode(path);

    const payload = new Uint8Array(saltBytes.length + pathBytes.length);
    payload.set(saltBytes);
    payload.set(pathBytes, saltBytes.length);

    const cryptoKey = await subtle.importKey(
        'raw',
        keyBytes.buffer,
        { name: 'HMAC', hash: 'SHA-256' },
        false,
        ['sign']
    );

    const digest = await subtle.sign('HMAC', cryptoKey, payload.buffer);
    return encoding.b64encode(new Uint8Array(digest), 'rawurl');
}
