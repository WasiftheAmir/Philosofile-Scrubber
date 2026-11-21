# philosofile-scrubber

Local-first, zero-cloud privacy scrubber exported from Philosofile.

`philosofile-scrubber` provides deterministic, on-device PII redaction for both text and images without external network calls or cloud inference:

- **Multilingual PII Text Scrubbing (`src-tauri/src/pipeline/scrubber.rs`):** Cross-platform (macOS, Windows, Linux) text redaction across 11 languages (`en`, `es`, `fr`, `de`, `pt`, `it`, `nl`, `ja`, `ko`, `hi`, `bn`), combining structured identifier detection, label-guided capture, and frequency-/dictionary-gated name recognition.
- **OS-Native Image Redaction (`src-tauri/src/pipeline/image_redact.rs`):** Detects faces and sensitive text via OS-native frameworks—Apple `Vision` on macOS (`13+`) and WinRT `Windows.Media.Ocr` / `Windows.Media.FaceAnalysis` on Windows—and applies mosaic pixelation to matched regions. On unsupported platforms (such as Linux), `redact_image` fails closed with `Err("unsupported platform")` and never returns unblurred image bytes.

## Architectural Overview

`philosofile-scrubber` is designed as a standalone, deterministic PII scrubbing engine tailored for embeddable desktop applications and offline privacy tooling. Its core architectural pillars include:

- **Confidence-Scored ID Registry:** Structured identification relies on `id_formats.csv` and `registry_check.rs`, implementing a multi-factor scoring pipeline that combines regex shape matching, proximity context bonuses, document-sensitivity clues, and real checksum validation (`luhn`, `verhoeff`, `mod11_nhs`, `mod97` IBAN, `aba`).
- **Script Normalization & Morphological Tokenization:** Text pre-processing incorporates script-specific Unicode normalization (such as NFC/NFKC handling and Bengali precomposed character normalization) coupled with lazy-loaded zstd-compressed dictionaries and Lindera/IPADIC (`mecab-ipadic`) morphological segmentation for Japanese (`ja`).
- **SHA-256 Verified Resource Manifests:** Every language directory under `src-tauri/resources/<lang>/` includes a `manifest.json` recording file roles, upstream sources, licenses, and SHA-256 digests verified by the test suite to ensure dataset integrity.

### Repository Layout

The `src-tauri/src/pipeline/` and `src-tauri/resources/` directory structure intentionally mirrors the production Philosofile desktop app 1:1 for transparent auditing and diffing:

```text
.
├── Cargo.toml / Cargo.lock          # Standalone Rust crate (philosofile_scrubber)
├── src-tauri/src/pipeline/
│   ├── scrubber.rs                  # Core text scrubbing engine & manifest loaders
│   ├── scrubber_tests.rs            # Unit & integration test suite
│   ├── registry_check.rs            # Registry & checksum validator helpers
│   ├── image_redact.rs              # OS-native OCR & face mosaic redaction
│   ├── id_formats.csv               # Embedded ID pattern & checksum registry
│   ├── label_groups.tsv             # Embedded multilingual label rules
│   └── allowlist.txt                # False-positive allowlist
└── src-tauri/resources/<lang>/      # Per-language word/name lists, NOTICE_<lang>.md, manifest.json
```

## Quick Start & Testing

Requires Rust `1.80.0` or later (and platform build prerequisites for image redaction on macOS/Windows):

```bash
cargo test
```

For detailed build requirements, testing options, known limitations, and licensing details, consult:
- [INSTRUCTIONS.md](INSTRUCTIONS.md)
- [KNOWN_ISSUES.md](KNOWN_ISSUES.md)
- [LICENCE_STATUS.md](LICENCE_STATUS.md)
- [src-tauri/THIRD_PARTY_NOTICES.md](src-tauri/THIRD_PARTY_NOTICES.md)

## Licensing

`philosofile-scrubber` uses a three-part licensing model separating the scrubber source code, project-authored/AI-generated data, and third-party linguistic datasets:

1. **Source Code & Project-Authored Pipeline Assets (`PolyForm Noncommercial License 1.0.0`):**
   All Rust source code and project-authored pipeline configuration/documentation (`src-tauri/src/pipeline/*.rs` including `scrubber_tests.rs`, `label_groups.tsv`, `id_formats.csv`, `Cargo.toml`, `Cargo.lock`, `README.md`, `INSTRUCTIONS.md`, `KNOWN_ISSUES.md`, `LICENCE_STATUS.md`) are source-available under the **PolyForm Noncommercial License 1.0.0** ([LICENSE](LICENSE)).

2. **Philosofile / Gemini AI-Generated Data (`CC BY 4.0`):**
   All project-authored and Gemini AI-generated name/word supplements and romanization expansions (including `bn_names_gemini.tsv`, `bn_romanized.txt`, `manual_verification` rows in `bn_names.tsv`, Hindi `hi_names_male_clean.csv` / `hi_names_female_clean.csv` / `hi_surnames_clean.csv`, Portuguese `pt_names.txt`, Spanish `es_surnames_ai_supplement.csv` and project-authored portions of `es_name_gated.txt`, Korean project-authored additions in `ko_names.txt` / `ko_surnames.txt` / `ko_names_romanized.txt`, Japanese project-generated expansions in `ja_given_romanized.txt` / `ja_surnames_romanized.txt`, and English project additions in `en_words_full.txt` / `en_names.txt`) are licensed under **Creative Commons Attribution 4.0 International (`CC BY 4.0`)**.

3. **Third-Party Linguistic Datasets:**
   Bundled third-party dictionaries, frequency lists, and name registries remain under their respective upstream licenses (`CC0 1.0`, `CC BY 4.0`, `CC BY-SA 4.0`, `LGPLv3`, `MPL 1.1`, `MPL 2.0`, `dl-de/by-2-0`, `IODL 2.0`, `Licence Ouverte v2.0`, Public Domain, etc.), as documented in [LICENCE_STATUS.md](LICENCE_STATUS.md), [src-tauri/THIRD_PARTY_NOTICES.md](src-tauri/THIRD_PARTY_NOTICES.md), and each language's `NOTICE_*.md` and `manifest.json`. Share-alike and weak-copyleft licenses on individual third-party data files apply strictly to those individual dataset files as separate data aggregations; the Rust scrubber source code itself is governed by the PolyForm Noncommercial License 1.0.0.
