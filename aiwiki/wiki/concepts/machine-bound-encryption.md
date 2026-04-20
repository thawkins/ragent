---
title: "Machine-Bound Encryption"
type: concept
generated: "2026-04-19T16:04:17.124757157+00:00"
---

# Machine-Bound Encryption

### From: mod

Machine-bound encryption is a security architecture pattern that cryptographically ties encrypted data to specific hardware and user contexts, ensuring that ciphertext cannot be decrypted on different machines or by different users even with access to the database file. In ragent storage, this is implemented through the MACHINE_KEY static, which derives a unique 32-byte encryption key from the combination of username (from USER or USERNAME environment variables) and home directory path using BLAKE3's derive_key function. This approach provides practical protection against credential theft through database file exfiltration, as decryption requires both the database and the original machine's specific identity material.

The encryption scheme uses a stream cipher construction where BLAKE3 in XOF mode generates a keystream from the machine key and a random nonce, which is then XORed with the plaintext API key. Each encryption generates a fresh random nonce, ensuring that identical credentials produce different ciphertexts and preventing pattern analysis. The v2 format encodes nonce and ciphertext together with a version prefix, enabling backward compatibility with older v1 obfuscation while supporting algorithmic upgrades. This design represents a pragmatic security trade-off: it protects against casual theft and remote attackers while acknowledging that sophisticated attackers with full system access could potentially extract keys from memory or intercept derivation inputs.

The security model assumes that the operating system and execution environment are trusted, which is appropriate for developer tools and local AI agents. The scheme deliberately avoids hardware security modules or TPM integration for portability, making it suitable for cross-platform deployment including containers and virtual machines where hardware binding would be impractical. Migration from v1 to v2 occurs transparently on credential access, with old entries automatically re-encrypted using the stronger scheme when read.

## External Resources

- [Key derivation function concepts on Wikipedia](https://en.wikipedia.org/wiki/Key_derivation_function) - Key derivation function concepts on Wikipedia
- [Stream cipher cryptographic primitive](https://en.wikipedia.org/wiki/Stream_cipher) - Stream cipher cryptographic primitive
- [Extendable-output functions in cryptography](https://en.wikipedia.org/wiki/Extendable-output_function) - Extendable-output functions in cryptography

## Sources

- [mod](../sources/mod.md)
