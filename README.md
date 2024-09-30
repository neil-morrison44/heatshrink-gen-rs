# heatshrink-gen-rs
a Rust generator based no_std heatshrink compression / decompression implementation

## Status
- The repo requires `gen_blocks` & `generic_const_exprs` which aren't yet stable (`gen_blocks` will be in Rust2024, due 2025) so it needs to be run on nightly.
- I'll publish it to crates.io etc once Rust2024 is out.
- Will need proper docs etc before that happens.
- It's currently only checked to be interoperable with itself. If anyone wants to PR fixes to make it interoperable with other more-correct implmentations, or just add tests to show that it already is, then feel free.
- Similarlly for benchmarks, I've not (yet) run any against more traditional HeatShrink implmentations, but anyone interested can PR them (or, better yet, a workflow that runs them automatically).
- This is a proof of concept for trying to get a minimal-memory `no_std` PNG decoder for `embedded_graphics` etc, for a use case where images are read from an SD card & the RAM isn't big enough to contain the raw compressed image data, the uncompressed data, the raw image data, and the framebuffer at once. HeatShrink is a simpler compression / decompression but the concept stands.

## Features
- Supports all sizes of lookahead / window, _probably should_ warn about ones that won't work but it currently tries them.
- Minimal memory usage, only needs the lookahead & the window (technically the lookahead is only used when encoding so there's an optimisation not yet made there) and a handful of other bytes to handle how the HeatShrink algorithm operates at both the bit and the byte level.
- All properties which were State Machines in the [original HeatShrink code](https://github.com/atomicobject/heatshrink) & the previous Rust implmentations I've come across are now handled by the generator code, which _I think_ increases readability for this kind of thing. The code itself could likely do with a readability pass or two.

## Usage

### Encoding

To encode you need an iter over `&u8`s, and to pick a window & lookahead size.
Here there's an 8 bit window (256 bytes), and a 4 bit lookahead (16 bytes).

```rust
let input = include_bytes!("./lib.rs");
let mut hs = <heatshrink!(8, 4)>::new();

let input_iter = (*input).iter();
let encode_iter = hs.encode(input_iter);
let encode_output: Vec<u8> = encode_iter.collect();
```


### Decode

To decode you again need an iter over `u8`s, and to pick a window & lookahead size.
Here there's an 8 bit window (256 bytes), and a 4 bit lookahead (16 bytes).
If you're reusing the same heatshrink instance, call `hs.reset()` on it to clear the window & lookahead to 0s.

```rust
let mut hs = <heatshrink!(8, 4)>::new();
let encode_output_iter = encode_output.iter();
let out = hs.decode(encode_output_iter);
let decoded_output: Vec<u8> = out.collect();
```
