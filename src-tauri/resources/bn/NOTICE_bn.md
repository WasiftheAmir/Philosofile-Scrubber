# Third-Party & Project-Authored Data Notices — Bangla (`bn`)

This directory contains Bangla word, suffix, name, and romanized-name lists loaded at runtime by Philosofile's PII scrubber.

## 1. Bangla Word List (`bn_words.tsv`) & Suffix List (`bn_suffixes.tsv`)

- **`bn_words.tsv`**:
  - **Source:** Combines Bangla Wiktionary page titles (`bnwiktionary_titles`), Bangla Wikipedia page titles (`bnwiki_titles`), and rows derived from `hermitdave/FrequencyWords` (`hermitdave_bn50k`, `https://github.com/hermitdave/FrequencyWords`), derived from OpenSubtitles via OPUS (`https://opus.nlpl.eu/`). Note that the `hermitdave/FrequencyWords` README states "MIT License for code" and "CC-by-sa-4.0 for content" — the content licence, **CC BY-SA 4.0**, applies to these rows.
  - **Licence:** **Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)** (`https://creativecommons.org/licenses/by-sa/4.0/`). See [`LICENSE_bn_words_CC-BY-SA-4.0.txt`](LICENSE_bn_words_CC-BY-SA-4.0.txt) and [`LICENSE_bn.txt`](LICENSE_bn.txt).
  - **Credit:** Subtitle data from opensubtitles.org (OpenSubtitles), via the OPUS corpus (https://opus.nlpl.eu/). Please cite: P. Lison and J. Tiedemann, 2016, OpenSubtitles2016: Extracting Large Parallel Corpora from Movie and TV Subtitles, LREC 2016.
  - **Open point:** Rights in the underlying subtitle texts have not been cleared. This is a known risk, accepted by the project maintainer.
  - **Share-alike:** Files containing these rows are distributed under CC BY-SA 4.0.
- **`bn_suffixes.tsv`**: Derived from Universal Dependencies Bengali-BRU under **CC BY-SA 4.0**.

## 2. Bangla Name Lists (`bn_names.tsv` and `bn_names_gemini.tsv`)

- **`bn_names.tsv` (`180` rows: `172` `wikipedia_category` rows, `8` `manual_verification` rows):**
  - **Wikipedia Subset (`source == "wikipedia_category"`, `172` rows):**
    - **Source:** Bangla Wikipedia category `বিষয়শ্রেণী:বাঙালি_ব্যক্তি` (`https://bn.wikipedia.org/wiki/বিষয়শ্রেণী:বাঙালি_ব্যক্তি`, direct members) — Wikimedia Foundation & contributors.
    - **Licence:** **Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)** (`https://creativecommons.org/licenses/by-sa/4.0/`).
  - **Manually Verified Entries (`source == "manual_verification"`, `8` rows):**
    - Curated validation names retained alongside the Wikipedia subset in `bn_names.tsv`.
- **`bn_names_gemini.tsv` (`2,344` `gemini_unverified` rows):**
  - Generated with Google Gemini, then curated and pruned by Wasif Amir. Licensed under CC BY 4.0. Changes: curated and pruned from the model's raw output.
  - See [`LICENSE_CC-BY-4.0.txt`](LICENSE_CC-BY-4.0.txt).

## 3. Romanized Bangla Names (`bn_romanized.txt`)

- **Source:** Extracted from the `romanized` column of the Gemini-generated Bangla entries (`bn_names_gemini.tsv`).
- **Authorship & Licence:** Generated with Google Gemini, then curated and pruned by Wasif Amir. Licensed under CC BY 4.0. Changes: curated and pruned from the model's raw output.
- See [`LICENSE_CC-BY-4.0.txt`](LICENSE_CC-BY-4.0.txt).
