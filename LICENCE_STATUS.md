# Licence Status

**No repository `LICENSE` file has been added yet.** The public release licence for Philosofile's scrubber code and project-authored / AI-generated data has not been decided. This repository MUST remain **private** until all items marked **Unsettled** below are resolved.

---

## Summary of Unsettled Items

1. **Rust source & project-authored configuration files** (`scrubber.rs`, `registry_check.rs`, `label_groups.tsv`, `id_formats.csv`, `Cargo.toml`, `README.md`): Public release licence is not yet chosen.
2. **AI-generated lists**:
   - **Bangla (`bn`)**: AI-generated subset of `bn_names.tsv` (`gemini_unverified` / `manual_verification` rows) and `bn_romanized.txt` (`project-authored, AI-generated (licence set with release licence)`).
   - **Hindi (`hi`)**: `hi_names_male_clean.csv`, `hi_names_female_clean.csv`, `hi_surnames_clean.csv` (Gemini-generated; `manifest.json` / `NOTICE_hi.md` mark `CC BY 4.0`, AI-generated provenance requires release decision).
   - **Portuguese (`pt`)**: `pt_names.txt` (generated lists; `manifest.json` records `"licence": "unknown"`).
   - **Spanish (`es`)**: `es_surnames_ai_supplement.csv` (`project-authored, AI-generated (licence set with release licence)`) and `es_name_gated.txt` (derived in part from `es_surnames_ai_supplement.csv`; `NOTICE_es.md` states its licence follows the project's release licence).
3. **Bangla word list (`bn/bn_words.tsv`)**: Licensed under `CC BY-SA 4.0` (`LICENSE_bn_words_CC-BY-SA-4.0.txt`), includes `hermitdave/FrequencyWords` (`hermitdave_bn50k`) rows built from OpenSubtitles via OPUS. As noted in `NOTICE_bn.md`, **rights in the underlying subtitle texts have not been cleared** (open point / accepted risk by maintainer). Also note `"version": "unknown"` in `manifest.json`.
4. **Korean (`ko`)**:
   - `ko_names.txt` and `ko_surnames.txt`: Wikidata base (`CC0 1.0`) plus 12 project-authored additions (`7` given names, `5` surnames) whose licence is `"set with the project's release licence"` (`manifest.json` `"version": "unknown"`).
   - `ko_names_romanized.txt`: Wikidata base (`CC0 1.0`) plus project-authored filtering and ASCII alias expansions (`NOTICE_ko.md`: governed by project's release licence).
5. **Spanish (`es`)**:
   - `es_names.txt`: Dual-sourced INE (`Ley 37/2007` / `CC BY 4.0`) and Wikidata (`CC0 1.0`), file header carries `# Status: UNREVIEWED (draft)`.
   - `es_surnames_ine.csv`: INE (`CC BY 4.0`, `Ley 37/2007` reuse terms).
   - `es_surnames_ai_supplement.csv` and `es_name_gated.txt`: Project release licence TBD + Leipzig Corpora `CC BY 4.0` / INE `CC BY 4.0` attribution (`es_name_gated.txt` derives from both INE/Leipzig and the AI supplement).
6. **Japanese (`ja`) project-generated expansions**: `ja_given_romanized.txt` and `ja_surnames_romanized.txt` combine Wikidata `CC0 1.0` bases with Philosofile-generated kana/ASCII alias expansions under Philosofile's project terms (release licence TBD). Native `ja_names.txt` and `ja_surnames.txt` also have `"version": "unknown"` in `manifest.json`.
7. **Other draft headers / unknown metadata**:
   - `de_names.txt`, `it_names.txt`, `fr_names.txt`, `nl_names.txt`: carry `# Status: UNREVIEWED (draft)` header comments (though their upstream licences are known: `dl-de/by-2-0`, `IODL 2.0`, `Licence Ouverte v2.0`, `CC0 1.0 / Open Overheid`).
   - Copyleft / weak-copyleft / share-alike interaction review pending public release choice: `bn_words.tsv` (`CC BY-SA 4.0`), `bn_names.tsv` Wikipedia subset (`CC BY-SA 4.0`), `bn_suffixes.tsv` (`CC BY-SA 4.0`), `fr_name_gated.txt` (`CC BY-SA 4.0`), `hi_words.tsv.zst` (`CC BY-SA 4.0`), `pt_BR_words.tsv` (`LGPLv3`), `es_words.txt` (`MPL 1.1`), `pt_PT_words.tsv` (`MPL 1.1`), `fr_words.tsv` (`MPL 2.0`).

---

## Complete Inventory of Code & Data Files

### Code & Pipeline Assets

| Path | Source | Recorded Licence | Status | Notes |
|---|---|---|---|---|
| `Cargo.toml` | Project-authored | None set (project release licence TBD) | **Unsettled** | Extracted standalone manifest for `scrubber.rs` |
| `Cargo.lock` | Cargo dependency lockfile | — | **Unsettled** | Generated lockfile for reproducible offline build |
| `src-tauri/src/pipeline/scrubber.rs` | Project-authored | None set (project release licence TBD) | **Unsettled** | Includes in-file unit tests |
| `src-tauri/src/pipeline/registry_check.rs` | Project-authored | None set (project release licence TBD) | **Unsettled** | Pattern validator module loaded by `scrubber.rs` |
| `src-tauri/src/pipeline/label_groups.tsv` | Project-authored | None set (project release licence TBD) | **Unsettled** | Embedded at compile time via `include_str!` |
| `src-tauri/src/pipeline/id_formats.csv` | Project-authored | None set (project release licence TBD) | **Unsettled** | Embedded at compile time via `include_str!` |
| `src-tauri/src/pipeline/allowlist.txt` | Microsoft Presidio (`presidio-analyzer/conf/default_recognizers.yaml`) | MIT (`LICENSE_allowlist.txt`, `THIRD_PARTY_NOTICES.md`) | **Settled** | Upstream MIT notice & full text included |
| `src-tauri/src/pipeline/LICENSE_allowlist.txt` | Microsoft Presidio | MIT | **Settled** | Licence text for `allowlist.txt` |
| `src-tauri/THIRD_PARTY_NOTICES.md` | Project-authored attribution index | — | **Settled** | Covers Lindera, IPADIC, Spanish Hunspell (`rla-es`), and Presidio `allowlist.txt` |

### Language Resource Data (`src-tauri/resources/<lang>/`)

#### Bangla (`bn`)

| File | Source | Licence (per `manifest.json` / `NOTICE_bn.md`) | Status | Notes |
|---|---|---|---|---|
| `bn/bn_words.tsv` | `bnwiktionary_titles`, `bnwiki_titles`, `hermitdave/FrequencyWords` (`hermitdave_bn50k`) via OpenSubtitles/OPUS | CC BY-SA 4.0 (`LICENSE_bn_words_CC-BY-SA-4.0.txt`, `LICENSE_bn.txt`) | **Unsettled** | Underlying OpenSubtitles subtitle rights have **not been cleared** (`NOTICE_bn.md` open point); `version: "unknown"` |
| `bn/bn_names.tsv` | Split: `bnwiki_category` (`172` rows), `gemini_unverified` (`2,344` rows), `manual_verification` (`8` rows) | `CC BY-SA 4.0` (Wikipedia subset); AI subset licence set with release licence | **Unsettled** | AI-generated subset licence undecided; `version: "unknown"` |
| `bn/bn_suffixes.tsv` | Universal Dependencies Bengali-BRU | CC BY-SA 4.0 | **Settled** | Third-party attribution recorded (`version: "unknown"`) |
| `bn/bn_romanized.txt` | `bn_names.tsv` (`romanized` column; AI-generated subset) | project-authored, AI-generated (licence set with release licence) | **Unsettled** | AI-generated; header notes `gemini_unverified` |
| `bn/manifest.json`, `bn/NOTICE_bn.md`, `bn/LICENSE_bn.txt`, `bn/LICENSE_bn_words_CC-BY-SA-4.0.txt` | Metadata & notices | — | **Unsettled** | `NOTICE_bn.md` / `LICENSE_bn.txt` reference open subtitle-rights point & release-licence dependency |

#### German (`de`)

| File | Source | Licence (per `manifest.json` / `NOTICE_de.md`) | Status | Notes |
|---|---|---|---|---|
| `de/de_names.txt` | Statistisches Bundesamt (Destatis) — *Vornamen und Familiennamen* | `dl-de/by-2-0` | **Unsettled** | File header carries `# Status: UNREVIEWED (draft)` alongside Destatis `dl-de/by-2-0` attribution & project transliterations |
| `de/manifest.json`, `de/NOTICE_de.md` | Metadata & notice | — | **Settled** | Clean upstream licence metadata |

#### English (`en`)

| File | Source | Licence (per `manifest.json` / `NOTICE_en.md`) | Status | Notes |
|---|---|---|---|---|
| `en/en_words_full.txt` | NLTK `words` corpus (Unix `words`) + 41 project business keywords | Public Domain (NLTK corpus; project additions under repository terms) | **Unsettled** | `41` project-authored additions governed by undecided project release terms (`NOTICE_en.md`) |
| `en/en_words_unix.txt` | Unix `words` via NLTK `words` corpus | Public Domain | **Settled** | Public Domain |
| `en/en_names.txt` | NLTK `names` corpus (Kantrowitz v1.3) + `smashew/NameDatabases` + 7 overlapping project extras | Mark Kantrowitz Names Corpus License (`LICENSE_en_names.txt`) / The Unlicense | **Unsettled** | Contains `7` project extras from `gen_dicts.py`; otherwise settled third-party attribution |
| `en/manifest.json`, `en/NOTICE_en.md`, `en/LICENSE_en_names.txt` | Metadata & notices | — | **Settled** | Upstream notices preserved |

#### Spanish (`es`)

| File | Source | Licence (per `manifest.json` / `NOTICE_es.md`) | Status | Notes |
|---|---|---|---|---|
| `es/es_words.txt` | LibreOffice / Apache OpenOffice Spanish dictionary (`rla-es` by Santiago Bosio, v2.8 / `es_ES`) | MPL 1.1 (triple-licensed MPL 1.1 / LGPL 3.0+ / GPL 3.0+) | **Settled** | Covered in `NOTICE_es.md` and `src-tauri/THIRD_PARTY_NOTICES.md` |
| `es/es_names.txt` | Instituto Nacional de Estadistica (INE) + Wikidata | INE reuse terms (`Ley 37/2007` / `CC BY 4.0`) + Wikidata `CC0 1.0` | **Unsettled** | Marked **Unsettled** (`es` mixed INE / Wikidata provenance + file header `# Status: UNREVIEWED (draft)`) |
| `es/es_surnames_ine.csv` | Instituto Nacional de Estadistica (INE) (`2025`, 4,425 rows) | `CC BY 4.0` (INE) / `Ley 37/2007` | **Unsettled** | INE attribution/non-distortion terms settled, grouped with `es` review (`LICENSE_es_names.txt` present) |
| `es/es_surnames_ai_supplement.csv` | Philosofile (project-authored, AI-generated, 275 rows) | project-authored, AI-generated (licence set with release licence) | **Unsettled** | AI-generated; release licence undecided |
| `es/es_name_gated.txt` | Derived from `es_surnames_ine.csv`, `es_surnames_ai_supplement.csv`, and Leipzig Corpora (`spa_news_2024_1M`, `spa_web_2016_1M`, `spa_wikipedia_2021_1M`) | Project release licence (`NOTICE_es.md`) + `CC BY 4.0` upstream attribution (Leipzig / INE) | **Unsettled** | Derives in part from AI supplement; release licence undecided |
| `es/manifest.json`, `es/NOTICE_es.md`, `es/LICENSE_es_names.txt` | Metadata & notices | — | **Settled** | Upstream notices & CC BY 4.0 text present |

#### French (`fr`)

| File | Source | Licence (per `manifest.json` / `NOTICE_fr.md`) | Status | Notes |
|---|---|---|---|---|
| `fr/fr_words.tsv` | Grammalecte Classique (`LibreOffice/dictionaries` `fr_FR`, v7.7) | MPL 2.0 | **Settled** | Attribution & license summary in `LICENSE_fr.txt`, `NOTICE_fr.md` (note: full MPL 2.0 text URL given; consider adding verbatim MPL 2.0 text before public release) |
| `fr/fr_name_gated.txt` | Derived from Hermit Dave `FrequencyWords` (`fr_50k`) vs INSEE `prenoms` | CC BY-SA 4.0 | **Unsettled** | Derived from `hermitdave/FrequencyWords` (OpenSubtitles-based, same upstream OpenSubtitles clearance context as `bn_words.tsv`) |
| `fr/fr_names.txt` | INSEE Fichier des prenoms via `ThinkR-open/prenoms` (`nat2021`) | Licence Ouverte v2.0 (Etalab) | **Unsettled** | File header carries `# Status: UNREVIEWED (draft)`; `NOTICE_fr.md` heading still references old path `src-tauri/src/pipeline/names/fr.txt` |
| `fr/manifest.json`, `fr/NOTICE_fr.md`, `fr/LICENSE_fr.txt` | Metadata & notices | — | **Unsettled** | Minor path staleness in `NOTICE_fr.md` heading (`fr.txt` vs `fr_names.txt`) |

#### Hindi (`hi`)

| File | Source | Licence (per `manifest.json` / `NOTICE_hi.md`) | Status | Notes |
|---|---|---|---|---|
| `hi/hi_words.tsv.zst` | English Wiktionary Hindi Extract via `kaikki.org` (`2026-10-30`) | CC BY-SA 4.0 | **Settled** | `LICENSE_hi_dictionary.txt` (CC BY-SA 4.0 full text) included |
| `hi/hi_freq.tsv.zst` | Leipzig Corpora Collection (`hin_mixed_2019_1M`, `hin_news_2022_1M`, `hin_wikipedia_2021_1M`) | CC BY 4.0 | **Settled** | `LICENSE_hi_names.txt` (CC BY 4.0 full text) + LREC'12 attribution included |
| `hi/hi_names_male_clean.csv` | Philosofile (Gemini-generated, cleaned) | CC BY 4.0 (`manifest.json`, `NOTICE_hi.md`) | **Unsettled** | AI-generated list (`version: "unknown"`); final public release / AI-data policy undecided |
| `hi/hi_names_female_clean.csv` | Philosofile (Gemini-generated, cleaned) | CC BY 4.0 (`manifest.json`, `NOTICE_hi.md`) | **Unsettled** | AI-generated list (`version: "unknown"`); final public release / AI-data policy undecided |
| `hi/hi_surnames_clean.csv` | Philosofile (Gemini-generated, cleaned) | CC BY 4.0 (`manifest.json`, `NOTICE_hi.md`) | **Unsettled** | AI-generated list (`version: "unknown"`); final public release / AI-data policy undecided |
| `hi/manifest.json`, `hi/NOTICE_hi.md`, `hi/LICENSE_hi_dictionary.txt`, `hi/LICENSE_hi_names.txt` | Metadata & notices | — | **Unsettled** | `NOTICE_hi.md` also mentions unbundled raw CSVs (`hi_names_male.csv` etc.) |

#### Italian (`it`)

| File | Source | Licence (per `manifest.json` / `NOTICE_it.md`) | Status | Notes |
|---|---|---|---|---|
| `it/it_names.txt` | Istituto Nazionale di Statistica (ISTAT) — *I nomi e cognomi più diffusi* | IODL 2.0 | **Unsettled** | File header carries `# Status: UNREVIEWED (draft)` |
| `it/manifest.json`, `it/NOTICE_it.md` | Metadata & notice | — | **Settled** | Clean upstream metadata |

#### Japanese (`ja`)

| File | Source | Licence (per `manifest.json` / `NOTICE_ja.md`) | Status | Notes |
|---|---|---|---|---|
| `ja/ja_names.txt` | Wikidata (`ja_names_joined.tsv`) | CC0 1.0 | **Settled** | Wikidata CC0 1.0 (`version: "unknown"` in `manifest.json`) |
| `ja/ja_surnames.txt` | Wikidata (`a7955fa3-query_4.csv`) | CC0 1.0 | **Settled** | Wikidata CC0 1.0 (`version: "unknown"` in `manifest.json`) |
| `ja/ja_given_romanized.txt` | Wikidata (`en`) + project-generated kana Hepburn/passport expansions | CC0 1.0 (Wikidata base) / Project-authored (kana expansions) | **Unsettled** | Project-authored expansions follow undecided release licence |
| `ja/ja_surnames_romanized.txt` | Wikidata (`en`) + project-generated ASCII/diacritic alias expansions | CC0 1.0 (Wikidata base) / Project-authored (ASCII alias expansions) | **Unsettled** | Project-authored expansions follow undecided release licence |
| `ja/ipadic/*.zst` (9 files: `char_def.bin.zst`, `dict.trie.zst`, `dict.vals.zst`, `dict.valsidx.zst`, `dict.words.zst`, `dict.wordsidx.zst`, `matrix.mtx.zst`, `metadata.json.zst`, `unk.bin.zst`) | `mecab-ipadic` (v`2.7.0-20070801`) via Lindera | IPADIC Notice (NAIST / ICOT Free Software) | **Settled** | Full `NOTICE_ipadic.md` and `THIRD_PARTY_NOTICES.md` included |
| `ja/manifest.json`, `ja/NOTICE_ja.md`, `ja/NOTICE_ipadic.md` | Metadata & notices | — | **Settled** | Notices complete |

#### Korean (`ko`)

| File | Source | Licence (per `manifest.json` / `NOTICE_ko.md`) | Status | Notes |
|---|---|---|---|---|
| `ko/ko_names.txt` | Wikidata (`CC0 1.0`) + `7` project-authored additions | `CC0 1.0` (Wikidata); project-authored additions licence set with release licence | **Unsettled** | Project-authored additions depend on undecided release licence (`version: "unknown"`) |
| `ko/ko_surnames.txt` | Wikidata (`CC0 1.0`) + `5` project-authored additions | `CC0 1.0` (Wikidata); project-authored additions licence set with release licence | **Unsettled** | Project-authored additions depend on undecided release licence (`version: "unknown"`) |
| `ko/ko_names_romanized.txt` | Wikidata (`581fb123-1794212598643_query_3.csv`) + project-authored filtering & alias expansions | `CC0 1.0` (Wikidata base); filtering/expansions governed by project release licence (`NOTICE_ko.md`) | **Unsettled** | Project-authored expansions governed by undecided release licence |
| `ko/manifest.json`, `ko/NOTICE_ko.md` | Metadata & notice | — | **Settled** | Accurately documents split Wikidata CC0 + project-authored terms |

#### Dutch (`nl`)

| File | Source | Licence (per `manifest.json` / `NOTICE_nl.md`) | Status | Notes |
|---|---|---|---|---|
| `nl/nl_names.txt` | Sociale Verzekeringsbank (SVB) / CBS | CC0 1.0 / Open Overheid | **Unsettled** | File header carries `# Status: UNREVIEWED (draft)` |
| `nl/manifest.json`, `nl/NOTICE_nl.md` | Metadata & notice | — | **Settled** | Clean upstream metadata |

#### Portuguese (`pt`)

| File | Source | Licence (per `manifest.json` / `NOTICE_pt.md`) | Status | Notes |
|---|---|---|---|---|
| `pt/pt_BR_words.tsv` | VERO (Verificador Ortográfico Livre) `pt_BR` v3.2.15 | LGPLv3 | **Settled** | Full `LICENSE_pt_BR.txt` included |
| `pt/pt_PT_words.tsv` | Hunspell `pt_PT` Dictionary (Projecto Natura, v20251001) | MPL 1.1 (elected from GPLv2 / LGPLv2.1 / MPL 1.1) | **Settled** | Full `LICENSE_pt_PT.txt` included |
| `pt/pt_name_gated.txt` | Leipzig Corpora vs Portuguese name lists | CC BY 4.0 (`manifest.json`, `NOTICE_pt.md`) | **Unsettled** | `version: "unknown"`; generated from `pt_names.txt` whose licence is `unknown` |
| `pt/pt_names.txt` | Philosofile generated Portuguese name lists (`male_first_names.csv`, `female_first_names.csv`, `surnames.csv`) | `unknown` (`manifest.json`) | **Unsettled** | Generated / AI-origin list; explicit `"licence": "unknown"` |
| `pt/freq/pt_freq.tsv.zst` | Leipzig Corpora Collection (`por-pt_web_2015_1M`, `por-br_newscrawl_2011_1M`, `por_news_2024_1M`) | CC BY 4.0 | **Settled** | LREC'12 attribution in `NOTICE_pt.md` (note: read only by `#[cfg(test)]` unit test, not runtime scrubbing) |
| `pt/manifest.json`, `pt/NOTICE_pt.md`, `pt/LICENSE_pt_BR.txt`, `pt/LICENSE_pt_PT.txt` | Metadata & notices | — | **Unsettled** | `NOTICE_pt.md` heading still references `src-tauri/src/pipeline/names/pt.txt` (`pt_names.txt`) |
