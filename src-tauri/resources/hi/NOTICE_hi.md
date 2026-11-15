# Hindi Language Resources Attribution & Notice

This directory contains the external language data and name resources used by the Philosofile Hindi PII scrubber pipeline.

## License Separation

- **Rust source code (`src-tauri/src/`)**: Retains the Philosofile project's existing repository licence. The CC BY-SA 4.0 dictionary word list (`hi_words.tsv.zst`) is loaded and decompressed lazily at runtime from bundle resources and is **not** compiled into the application binary as source code.
- **Hindi Wiktionary word list (`hi_words.tsv.zst`)**: Licensed under **Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)** (full licence text in `LICENSE_hi_dictionary.txt`; copyright notice also included in the compressed file header).
- **Hindi combined frequency counts (`hi_freq.tsv.zst`)**: Licensed under **Creative Commons Attribution 4.0 International (CC BY 4.0)** (full licence text in `LICENSE_hi_names.txt`).
- **Hindi name lists (`hi_names_male.csv`, `hi_names_female.csv`, `hi_surnames.csv`, and cleaned lists `hi_names_male_clean.csv`, `hi_names_female_clean.csv`, `hi_surnames_clean.csv`)**: Project-owned generated lists licensed under **Creative Commons Attribution 4.0 International (CC BY 4.0)** (full licence text in `LICENSE_hi_names.txt`), kept separate from the CC BY-SA 4.0 word list.

---

## 1. Hindi Dictionary Word List (`hi_words.tsv.zst`)
- **File**: `hi_words.tsv.zst` (85,013 unique normalized Devanagari forms, zstd-compressed TSV)
- **Licence**: **CC BY-SA 4.0** (`LICENSE_hi_dictionary.txt`)
- **Source**: **English Wiktionary Hindi Extract**
  - File: `kaikki.org-dictionary-Hindi.jsonl` via **kaikki.org / wiktextract** (extracted from Wiktionary)
  - URL: `https://kaikki.org/dictionary/Hindi/`
  - **kaikki.org extraction date**: `2026-10-30`
  - **Wiktionary dump date**: `2026-10-01`
  - **Licence**: **CC BY-SA 4.0**
  - **Processing**: Kept entries with `pos != "name"` (36,125 of 39,513 JSONL entries; 2,808 name-only entries excluded). Extracted base `word` and `forms[].form`, filtered to strict Devanagari (`U+0900..U+097F`, dropping Devanagari digits `०-९`, danda `। ॥`, non-Devanagari characters, and leading combining marks), and normalized (Unicode NFC, precomposed nukta consonants `ऩ`/`ऱ`/`ऴ` decomposed to `base + U+093C`, post-vowel nukta reordered before dependent vowel signs, recomposed to NFC; anusvara `ं` and chandrabindu `ँ` kept distinct). Leipzig corpus tokens are **not** included in `hi_words.tsv.zst`.

---

## 2. Hindi Word Frequency Table (`hi_freq.tsv.zst`)
- **File**: `hi_freq.tsv.zst` (590,152 unique normalized Devanagari tokens; total summed count 47,846,670)
- **Licence**: **CC BY 4.0** (`LICENSE_hi_names.txt`)
- **Sources**: **Leipzig Corpora Collection — Hindi** (used for frequency counts only)
  - Corpora used:
    - `hin_mixed_2019_1M` (Hindi Mixed 2019, 1M sentences)
    - `hin_news_2022_1M` (Hindi News 2022, 1M sentences)
    - `hin_wikipedia_2021_1M` (Hindi Wikipedia 2021, 1M sentences)
  - URL: `https://wortschatz.uni-leipzig.de/en/download/Hindi`
  - Attribution: D. Goldhahn, T. Eckart & U. Quasthoff: *Building Large Monolingual Dictionaries at the Leipzig Corpora Collection: From 100 to 200 Languages.* LREC 2012.
  - Processing: Summed raw counts across the three `*-words.txt` files only (sentence files were not used), filtered to strict Devanagari tokens and normalized identically to `hi_words.tsv.zst`.

---

## 3. Hindi Name Lists (`hi_names_*.csv` & `hi_*_clean.csv`)
- **Origin & Licence**: Generated with Google Gemini, then curated and pruned by Wasif Amir. Licensed under CC BY 4.0. Changes: curated and pruned from the model's raw output.
- **Full Licence Text**: [`LICENSE_CC-BY-4.0.txt`](LICENSE_CC-BY-4.0.txt) (also in [`LICENSE_hi_names.txt`](LICENSE_hi_names.txt)).
- **Original CSVs** (kept unmodified):
  - `hi_names_male.csv` (1,500)
  - `hi_names_female.csv` (1,500)
  - `hi_surnames.csv` (3,872)
- **Cleaned CSVs** (53 Wiktionary verb/function-tag entries, 27 place-name entries, 4 Wiktionary `adv`-without-`name` entries, and `वाले` excluded and logged outside the bundle):
  - `hi_names_male_clean.csv` (1,485)
  - `hi_names_female_clean.csv` (1,474)
  - `hi_surnames_clean.csv` (3,822)
  - Unique union across cleaned lists: 6,629 entries.
- **Scope note**: Covers Devanagari script names only. Romanized Hindi names are out of scope for this component.
