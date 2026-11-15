# Build and Test Instructions

`philosofile-scrubber` contains two subsystems:
1. **Text scrubber (`src-tauri/src/pipeline/scrubber.rs`)**: cross-platform (macOS, Windows, Linux).
2. **Image redaction (`src-tauri/src/pipeline/image_redact.rs`)**: OS-native OCR and face mosaic blurring (**macOS and Windows only**). On all other platforms, `redact_image` fails closed with `Err("unsupported platform")` and never returns unblurred image bytes.

---

## Prerequisites

- **Rust toolchain:** Rust `1.77.2` or later (per `rust-version = "1.77.2"` in `Cargo.toml`).

### macOS
- **OS version:** macOS **13 (Ventura)** or later (`apple-vision` v0.16.7 specifies `.macOS(.v13)` in its Swift bridge package).
- **System frameworks:** Apple `Vision`, `CoreGraphics`, `CoreImage`, and `Foundation` are built into macOS.
- **Build tools:** Xcode or Xcode Command Line Tools (`xcode-select --install`) providing `swift` / Swift tools (`swift-tools-version:5.9`) so `apple-vision`'s build script can compile its static `VisionBridge` library.

### Windows
- **OS version:** Windows 10 (build 1809+ / Anniversary+ with WinRT Media OCR & FaceAnalysis) or Windows 11.
- **Toolchain:** Rust `x86_64-pc-windows-msvc` (or `aarch64-pc-windows-msvc`) with the Windows 10/11 SDK and MSVC Build Tools.
- **OCR runtime requirement:** `OcrEngine::TryCreateFromUserProfileLanguages()` requires at least one Windows OCR language pack installed (Settings → Time & Language → Language & region).

### Linux / Other Platforms
- **Image redaction:** **Unsupported.** `image_redact::redact_image` compiles without platform OCR/face dependencies and returns `Err("unsupported platform")`.
- **Text scrubber:** Fully supported.

---

## Building and Running Tests

All resource paths resolve relative to `CARGO_MANIFEST_DIR` (`src-tauri/resources/...`).

### Check and test (offline when dependencies are cached)
```bash
cargo check --offline
cargo test --offline
```

### Run image redaction tests specifically
- **macOS:**
  ```bash
  cargo test image_redact::tests
  ```
- **Windows:**
  ```powershell
  cargo test image_redact::tests
  ```
- **Linux / other platforms (verifies fail-closed `Err("unsupported platform")` + pure helpers):**
  ```bash
  cargo test image_redact::tests
  ```

### Run ignored fixture tests manually
```bash
cargo test image_redact::tests -- --ignored
```

---

## Testing the image path on your own images

Set `PHILOSOFILE_TEST_IMAGE` to the path of a local photo containing a face and visible sensitive text, then run the pixel verification test:

```bash
PHILOSOFILE_TEST_IMAGE=/path/to/local_photo.jpg cargo test image_redact::tests::test_redact_image_env_pixel_verification -- --nocapture
```

- The image stays strictly local and is never copied or committed (paths matching `scratch/internal_test/`, `*.test.jpg`, and `*.test.png` are gitignored).
- The test prints only counts, box coordinates, and changed-pixel counts — never image bytes or detected text.
- On Windows, the WinRT OCR / `FaceAnalysis` path requires running on Windows with the MSVC toolchain; non-Windows runs mark Windows coverage as skipped.

---

## Licence

- **Scrubber code (`src-tauri/src/pipeline/*.rs`, `label_groups.tsv`, `id_formats.csv`, `Cargo.toml`):** Licensed under **PolyForm Noncommercial License 1.0.0** (`PolyForm-Noncommercial-1.0.0`; see [`LICENSE`](LICENSE)) for **non-commercial use only**. The code is **source-available, not open source**.
- **Data folders (`src-tauri/resources/<lang>/`):** Each language data folder keeps its own separate licence (such as `CC BY 4.0` for Gemini-generated name lists, Leipzig Corpora, and INE data; `CC BY-SA 4.0` for Bangla and Hindi dictionaries and Bangla Wikipedia rows; `CC0 1.0`; `MPL 1.1`; `MPL 2.0`; and `LGPLv3`), as listed in [`src-tauri/THIRD_PARTY_NOTICES.md`](src-tauri/THIRD_PARTY_NOTICES.md) and each folder's `NOTICE_*.md` and `manifest.json`.
