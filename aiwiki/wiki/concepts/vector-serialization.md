---
title: "Vector Serialization"
type: concept
generated: "2026-04-19T21:45:52.711857078+00:00"
---

# Vector Serialization

### From: embedding

Vector serialization in this module addresses the challenge of storing high-dimensional floating-point embeddings in SQLite's type system, which lacks a native array type. The serialization strategy converts Vec<f32> to compact byte blobs using little-endian IEEE 754 binary32 format, producing 4 bytes per dimension. This approach leverages SQLite's BLOB type for efficient storage while maintaining bit-exact reconstruction through controlled deserialization. The choice of little-endian reflects x86_64 dominance in deployment targets and aligns with SQLite's internal endianness conventions on those platforms.

The serialization implementation uses `Vec::with_capacity` to pre-allocate the exact required buffer size, avoiding reallocations during the single-pass conversion. The `extend_from_slice` method efficiently appends each f32's byte representation, with `to_le_bytes` handling the platform-independent endianness conversion. Deserialization reverses this process using `chunks_exact` to iterate over 4-byte segments, reconstructing f32 values via `from_le_bytes`. Error handling validates blob length against expected dimensions, preventing buffer overruns and detecting data corruption with descriptive error messages via anyhow.

This serialization design balances multiple concerns: storage efficiency (no text parsing overhead), query performance (fixed-size BLOBs enable SQLite optimizations), and correctness (exact roundtrip preservation). The 4-byte-per-float overhead is minimal compared to text alternatives like JSON or CSV, which would require variable-length parsing and significant size inflation. The explicit dimension parameter in deserialization provides defense in depth, catching bugs where stored embeddings might be interpreted with wrong dimensionality. For the default 384-dimensional embeddings, each vector occupies exactly 1536 bytes, enabling straightforward capacity planning and index design for embedding tables.

## External Resources

- [SQLite Datatypes documentation](https://www.sqlite.org/datatype3.html) - SQLite Datatypes documentation
- [Rust f32 primitive documentation including to_le_bytes](https://doc.rust-lang.org/std/primitive.f32.html) - Rust f32 primitive documentation including to_le_bytes

## Sources

- [embedding](../sources/embedding.md)
