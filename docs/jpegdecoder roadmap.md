# Building a Zero-Dependency JPEG Decoder in Rust

> **Status (2026-06-06): the "Support First" subset is implemented** in
> `src/canvas/svg_rasterizer.rs` (`decode_jpeg`) for inline `data:` JPEG images,
> wired through the SVG R5 image path. Implemented: baseline / extended-sequential
> Huffman (SOF0/SOF1), 8-bit, grayscale + YCbCr, 4:4:4 / 4:2:2 / 4:2:0, restart
> markers, `0xFF00` de-stuffing, separable float IDCT. Still deferred (the "Defer
> Initially" list below): progressive, arithmetic, CMYK/YCCK, 12-bit, lossless —
> all diagnosed `image.unsupported_jpeg`. A future optimization is the integer/AAN
> IDCT. See `docs/SVG_RENDERER_ROADMAP.md` R5 for the authoritative status.

JPEG is a family of formats, so a scratch decoder should begin with the most common subset.

## Support First

- Baseline DCT JPEG
- 8-bit samples
- Huffman entropy coding
- Grayscale
- YCbCr/RGB color spaces
- Common chroma subsampling:
  - 4:4:4
  - 4:2:2
  - 4:2:0

## Defer Initially

- Progressive JPEG
- Arithmetic coding
- CMYK/YCCK
- Exotic restart behavior
- 12-bit JPEG
- Lossless JPEG

---

# Architecture in Rust

```text
jpeg/
├── mod.rs
├── marker.rs        // SOI, APPn, DQT, DHT, SOF0, SOS, DRI, EOI
├── bitstream.rs     // bit reader, byte stuffing 0xFF00
├── huffman.rs       // canonical Huffman decode tables
├── quant.rs         // quantization tables
├── idct.rs          // inverse DCT
├── color.rs         // YCbCr -> RGB
├── upsample.rs      // chroma upsampling
└── decoder.rs       // public API
```

---

# Core Pipeline

```text
bytes
  -> parse markers
  -> read quantization tables
  -> read Huffman tables
  -> read frame header
  -> read scan header
  -> entropy decode MCUs
  -> dequantize 8x8 blocks
  -> inverse DCT
  -> upsample components
  -> color convert
  -> RGB/RGBA output
```

---

# Public API

```rust
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>, // RGB, 3 bytes per pixel
}

pub enum JpegError {
    Format(&'static str),
    Unsupported(&'static str),
    Eof,
}

pub fn decode_jpeg(data: &[u8]) -> Result<Image, JpegError> {
    Decoder::new(data).decode()
}
```

---

# The Hardest Pieces

## 1. Marker Parsing

JPEG files are segment-based.

You scan for `0xFF` markers such as:

- `SOI`
- `DQT`
- `DHT`
- `SOF0`
- `SOS`
- `EOI`

---

## 2. Entropy Decoding

JPEG uses Huffman-coded coefficient deltas.

- DC coefficients are differential encoded.
- AC coefficients use run-length encoding.
- AC values are stored in zigzag order.

---

## 3. MCU Layout

Components may have different horizontal and vertical sampling factors.

These determine how many `8×8` blocks are decoded per MCU (Minimum Coded Unit).

---

## 4. IDCT

Start with a simple floating-point `8×8` IDCT.

Later, replace it with an optimized integer AAN IDCT for speed.

---

## 5. Upsampling and Color Conversion

For common JPEGs:

1. Decode Y, Cb, and Cr planes.
2. Upsample Cb and Cr to full resolution.
3. Convert YCbCr to RGB.

```rust
fn ycbcr_to_rgb(y: i32, cb: i32, cr: i32) -> [u8; 3] {
    let cb = cb - 128;
    let cr = cr - 128;

    let r = y + ((91881 * cr) >> 16);
    let g = y - ((22554 * cb + 46802 * cr) >> 16);
    let b = y + ((116130 * cb) >> 16);

    [
        r.clamp(0, 255) as u8,
        g.clamp(0, 255) as u8,
        b.clamp(0, 255) as u8,
    ]
}
```

---

# Staged Implementation Plan

1. Parse:
   - SOI
   - APPn
   - DQT
   - SOF0
   - DHT
   - SOS
   - EOI

2. Decode grayscale baseline JPEG.

3. Add `4:4:4` YCbCr support.

4. Add `4:2:0` and `4:2:2` subsampling.

5. Add restart marker support.

6. Improve IDCT implementation.

7. Add progressive JPEG support only after baseline decoding is solid.

---

# Zero-Dependency Design

Avoid `std`-heavy abstractions where possible.

Keep the decoder mostly slice-based:

```rust
pub struct Decoder<'a> {
    data: &'a [u8],
    pos: usize,
    qtables: [Option<[u16; 64]>; 4],
    htables: HuffmanTables,
    frame: Option<FrameInfo>,
}
```

---

# Testing Strategy

Compare output against known-good JPEG decoders using a corpus of small test images covering:

- Grayscale
- 4:4:4
- 4:2:2
- 4:2:0
- Restart markers
- Odd image dimensions
- Malformed files

The goal is bit-exact or visually identical output.

---

# Recommended Development Order

```text
parser
  -> grayscale baseline
  -> IDCT
  -> color conversion
  -> subsampling
  -> restart markers
  -> fuzzing
```

This order minimizes complexity while continuously producing valid, testable output.