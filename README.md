# philosofile-scrubber

Privacy scrubber for Philosofile.

## Licensing

`philosofile-scrubber` uses a three-part licensing model separating the scrubber source code, project-authored/AI-generated data, and third-party linguistic datasets:

1. **Source Code & Project-Authored Pipeline Assets (`PolyForm Noncommercial License 1.0.0`):**
   All Rust source code and project-authored pipeline configuration/documentation (`src-tauri/src/pipeline/*.rs`, `label_groups.tsv`, `id_formats.csv`, `Cargo.toml`, `Cargo.lock`, `README.md`, `INSTRUCTIONS.md`, `KNOWN_ISSUES.md`) are source-available under the **PolyForm Noncommercial License 1.0.0** ([LICENSE](LICENSE)).

2. **Philosofile / Gemini AI-Generated Data (`CC BY 4.0`):**
   All project-authored and Gemini AI-generated name/word supplements and romanization expansions (including `bn_names_gemini.tsv`, `bn_romanized.txt`, `manual_verification` rows in `bn_names.tsv`, Hindi `hi_names_male_clean.csv` / `hi_names_female_clean.csv` / `hi_surnames_clean.csv`, Portuguese `pt_names.txt`, Spanish `es_surnames_ai_supplement.csv` and project-authored portions of `es_name_gated.txt`, Korean project-authored additions in `ko_names.txt` / `ko_surnames.txt` / `ko_names_romanized.txt`, Japanese project-generated expansions in `ja_given_romanized.txt` / `ja_surnames_romanized.txt`, and English project additions in `en_words_full.txt` / `en_names.txt`) are licensed under **Creative Commons Attribution 4.0 International (`CC BY 4.0`)**.

3. **Third-Party Linguistic Datasets:**
   Bundled third-party dictionaries, frequency lists, and name registries remain under their respective upstream licenses (`CC0 1.0`, `CC BY 4.0`, `CC BY-SA 4.0`, `LGPLv3`, `MPL 1.1`, `MPL 2.0`, `dl-de/by-2-0`, `IODL 2.0`, `Licence Ouverte v2.0`, Public Domain, etc.), as documented in [LICENCE_STATUS.md](LICENCE_STATUS.md), [src-tauri/THIRD_PARTY_NOTICES.md](src-tauri/THIRD_PARTY_NOTICES.md), and each language's `NOTICE_*.md` and `manifest.json`. Share-alike and weak-copyleft licenses on individual third-party data files apply strictly to those individual dataset files as separate data aggregations; the Rust scrubber source code itself is governed by the PolyForm Noncommercial License 1.0.0.
