# ed2k

Streaming [ed2k](https://wiki.anidb.net/Ed2k-hash) hashing for any `Read`, with
crc32, md5 and sha1 computed in the same single pass.

- `hash_reader` / `hash_file` read through a fixed 1 MiB buffer and feed only the
  hashers selected with `Select`. ed2k is always computed.
- `ed2k_link` formats an `ed2k://|file|...|/` link.

The ed2k hash is MD4 over 9,728,000-byte blocks. A single block hashes to its
own digest. Multiple blocks hash to the MD4 of the concatenated block digests.
A file that is an exact multiple of the block size gets no trailing empty block.

```rust
use ed2k::{hash_file, Select};

let hashes = hash_file("episode.mkv".as_ref(), Select { crc32: true, md5: true, sha1: true })?;
println!("{}", ed2k::ed2k_link("episode.mkv", hashes.size, &hashes.ed2k));
# Ok::<(), std::io::Error>(())
```

Licensed under MIT.
