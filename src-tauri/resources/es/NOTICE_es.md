# Third-Party & Project-Authored Data Notices — Spanish (`es`)

This directory contains Spanish word, given-name, and surname lists loaded at runtime by Philosofile's PII scrubber.

## 1. Given Name List (`es_names.txt`)

- **Source:** Instituto Nacional de Estadistica (INE) — *Frecuencias de nombres y apellidos* (`https://www.ine.es/dyngs/INEbase/es/operacion.htm?c=Estadistica_C&cid=1254736177009`) and Wikidata (`https://www.wikidata.org/`, `query (2).csv`).
- **Version / Date:** `2026-11-02` (header status: `UNREVIEWED (draft)`).
- **Licence:** INE reuse terms (`Ley 37/2007` / `CC BY 4.0`) and Wikidata `CC0 1.0 Universal`.
  - CC BY 4.0 canonical URL: https://creativecommons.org/licenses/by/4.0/
  - CC BY 4.0 full license text: [`LICENSE_es_names.txt`](LICENSE_es_names.txt)
  - CC0 1.0 canonical URL: https://creativecommons.org/publicdomain/zero/1.0/
- **Project-authored changes:** Extracted and formatted for Philosofile PII detection (header comments and single-entry-per-line normalization; no upstream attribution requirements removed).

## 2. INE Spanish Surname List (`es_surnames_ine.csv`, `4,425` rows)

- **Source:** Instituto Nacional de Estadistica (INE) — *Estadística del Padrón Continuo / Apellidos más frecuentes* (`https://www.ine.es/dyngs/INEbase/es/operacion.htm?c=Estadistica_C&cid=1254736177009`) and `INE_apellidos_por_nacionalidad_2025_GUINEA_ECUATORIAL` (`4,410` INE surnames with `>= 1,000` first-surname holders plus `15` Equatorial Guinean surnames).
- **Version:** `2025` (`es_surnames_ine_v1.csv`).
- **Licence:** **CC BY 4.0 (INE)** (`https://creativecommons.org/licenses/by/4.0/`, full text in [`LICENSE_es_names.txt`](LICENSE_es_names.txt)).
- **INE Reuse Policy (Aviso Legal / Ley 37/2007):** INE authorizes commercial and non-commercial reuse of its disseminated data provided the source (Instituto Nacional de Estadística — INE) is cited, the original sense/meaning of the data is not distorted or altered, and no official INE sponsorship or endorsement of the derived work is implied (https://www.ine.es/ss/Satellite?c=Page&cid=1254735849170&pagename=Ayuda%2FINELayout&L=1).

## 3. AI-Generated Spanish Surname Supplement (`es_surnames_ai_supplement.csv`, `275` rows)

- **Source:** Generated with Google Gemini, then curated and pruned by Wasif Amir. Licensed under CC BY 4.0. Changes: curated and pruned from the model's raw output.
- **Full Licence Text:** [`LICENSE_CC-BY-4.0.txt`](LICENSE_CC-BY-4.0.txt) (`https://creativecommons.org/licenses/by/4.0/`).

## 4. Spanish Word List (`es_words.txt`)

- **Source:** Spanish Hunspell Dictionary (`rla-es` / LibreOffice, `https://github.com/sbosio/rla-es`), Version `2.8 / es_ES`.
- **Licence:** Mozilla Public License Version 1.1 (`MPL-1.1`) / GNU LGPL v3.0+ / GNU GPL v3.0+ (bundled under `MPL-1.1`). See `src-tauri/THIRD_PARTY_NOTICES.md` for full notice and `https://www.mozilla.org/MPL/MPL-1.1.html`.

## 5. Spanish Cue-Gated Surname List (`es_name_gated.txt`, `185` entries)

- **Content & Derivation:** Derived from three sources: INE (`es_surnames_ine.csv`), `es_surnames_ai_supplement.csv` (Generated with Google Gemini, then curated and pruned by Wasif Amir), and surface-form frequency ratios against the Leipzig Spanish corpora (`log10(name_cnt / word_cnt) < -1.0`), excluding protected entries (top 1,000 INE surnames by `first_surname_count`, length <= 3, and multi-word entries). Redacted only when cued by a label or salutation.
- **Corpora Used for Frequency Comparison (not bundled):**
  - `spa_news_2024_1M` — Spanish News 2024 (1M sentences), Leipzig Corpora Collection
  - `spa_wikipedia_2021_1M` — Spanish Wikipedia 2021 (1M sentences), Leipzig Corpora Collection
  - `spa_web_2016_1M` — Spanish Web 2016 (1M sentences), Leipzig Corpora Collection
  - URL: `https://wortschatz.uni-leipzig.de/en/download/Spanish`
- **Licence & Attribution:** Licensed under **CC BY 4.0** ([`LICENSE_CC-BY-4.0.txt`](LICENSE_CC-BY-4.0.txt), `https://creativecommons.org/licenses/by/4.0/`) with attribution to all three sources:
  1. **Instituto Nacional de Estadística (INE)** — *Estadística del Padrón Continuo / Apellidos más frecuentes* (`CC BY 4.0` / `Ley 37/2007`, non-distortion notice applies).
  2. **Leipzig Corpora Collection** — D. Goldhahn, T. Eckart & U. Quasthoff: *Building Large Monolingual Dictionaries at the Leipzig Corpora Collection: From 100 to 200 Languages.* In: Proceedings of the 8th Language Resources and Evaluation Conference (LREC'12), 2012 (`CC BY 4.0`).
  3. **`es_surnames_ai_supplement.csv`** — Generated with Google Gemini, then curated and pruned by Wasif Amir. Licensed under CC BY 4.0. Changes: curated and pruned from the model's raw output.
