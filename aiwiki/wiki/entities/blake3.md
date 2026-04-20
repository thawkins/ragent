---
title: "BLAKE3"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:04:17.123743014+00:00"
---

# BLAKE3

**Type:** technology

### From: mod

BLAKE3 is a modern cryptographic hash function that provides the security foundation for ragent's credential encryption system. The storage module utilizes BLAKE3 in multiple capacities: for key derivation using the derive_key function to create machine-specific encryption keys from user identity material, and in XOF (eXtendable-Output Function) mode to generate keystreams for stream cipher-style encryption of API credentials. The choice of BLAKE3 reflects contemporary cryptographic best practices, offering significantly faster performance than older algorithms like SHA-256 while maintaining equivalent or superior security guarantees.

The machine key derivation process combines environment variables (USER or USERNAME) with the home directory path as input material, creating a 32-byte key unique to each machine-user combination. This design provides practical security by binding encrypted credentials to specific execution environments, meaning that even if an attacker obtains the database file, decryption requires access to the same machine and user account. The XOF mode enables generation of arbitrary-length keystreams matched to the size of credentials being encrypted, implementing a secure XOR-based encryption scheme with nonce-based uniqueness for each encryption operation.

## External Resources

- [BLAKE3 official GitHub repository with specifications](https://github.com/BLAKE3-team/BLAKE3) - BLAKE3 official GitHub repository with specifications
- [blake3 crate documentation for Rust bindings](https://docs.rs/blake3/latest/blake3/) - blake3 crate documentation for Rust bindings
- [RFC 7693: The BLAKE2 Cryptographic Hash and Message Authentication Code](https://www.rfc-editor.org/rfc/rfc7693.html) - RFC 7693: The BLAKE2 Cryptographic Hash and Message Authentication Code

## Sources

- [mod](../sources/mod.md)
