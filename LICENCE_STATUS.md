# Licence Status

All source code, pipeline assets, project-authored/AI-generated datasets, and third-party linguistic resources in `philosofile-scrubber` are **Settled** under the three-part licensing model below:

1. **Source Code & Project-Authored Pipeline Assets — `PolyForm Noncommercial License 1.0.0`** ([`LICENSE`](LICENSE)):
   Covers all Rust source files (`src-tauri/src/pipeline/*.rs`), embedded pipeline configuration (`label_groups.tsv`, `id_formats.csv`), crate manifests (`Cargo.toml`, `Cargo.lock`), and root documentation (`README.md`, `INSTRUCTIONS.md`, `KNOWN_ISSUES.md`).
2. **Project-Authored & Gemini AI-Generated Datasets & Expansions — `CC BY 4.0`**:
   Covers Bangla AI-generated and manually verified rows (`bn_names_gemini.tsv`, `bn_romanized.txt`, and `manual_verification` rows in `bn_names.tsv`), Hindi (`hi_names_male_clean.csv`, `hi_names_female_clean.csv`, `hi_surnames_clean.csv`), Portuguese (`pt_names.txt`), Spanish (`es_surnames_ai_supplement.csv` and project-authored portions of `es_name_gated.txt`), Korean project-authored additions (`ko_names.txt`, `ko_surnames.txt`, `ko_names_romanized.txt`), Japanese project-generated expansions (`ja_given_romanized.txt`, `ja_surnames_romanized.txt`), and English project additions (`en_words_full.txt`, `en_names.txt`).
3. **Third-Party / Non-AI Linguistic Datasets — Respective Upstream Licenses**:
   Third-party dictionaries, frequency lists, and name registries retain their respective upstream licenses (`CC0 1.0`, `CC BY 4.0`, `CC BY-SA 4.0`, `LGPLv3`, `MPL 1.1`, `MPL 2.0`, `dl-de/by-2-0`, `IODL 2.0`, `Licence Ouverte v2.0`, Public Domain, and IPADIC Notice). Share-alike and weak-copyleft licenses on individual dataset files (`bn_words.tsv`, `bn_names.tsv` Wikipedia subset, `bn_suffixes.tsv`, `fr_name_gated.txt`, `hi_words.tsv.zst`, `pt_BR_words.tsv`, `es_words.txt`, `pt_PT_words.tsv`, `fr_words.tsv`) apply strictly to those individual dataset files as separate data aggregations loaded at runtime; the Rust source code itself is governed by the PolyForm Noncommercial License 1.0.0.

---

## Complete Inventory of Code & Data Files

### Code & Pipeline Assets

| Path | Source | Recorded Licence | Status | Notes |
|---|---|---|---|---|
| `LICENSE` | PolyForm Project | PolyForm Noncommercial 1.0.0 | **Settled** | Root repository licence text |
| `README.md` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Project overview and 3-part licensing summary |
| `INSTRUCTIONS.md` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Build, test, and licensing instructions |
| `KNOWN_ISSUES.md` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Documented scrubber and harness limitations |
| `Cargo.toml` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Standalone crate manifest (`license = "PolyForm-Noncommercial-1.0.0"`) |
| `Cargo.lock` | Cargo dependency lockfile | PolyForm Noncommercial 1.0.0 | **Settled** | Generated lockfile for reproducible offline build |
| `src-tauri/src/pipeline/scrubber.rs` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Core text scrubbing engine |
| `src-tauri/src/pipeline/scrubber_tests.rs` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Scrubber unit and integration test suite |
| `src-tauri/src/pipeline/registry_check.rs` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Pattern validator module loaded by `scrubber.rs` |
| `src-tauri/src/pipeline/image_redact.rs` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | OS-native OCR and face mosaic redaction module |
| `src-tauri/src/pipeline/label_groups.tsv` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Embedded at compile time via `include_str!` |
| `src-tauri/src/pipeline/id_formats.csv` | Project-authored | PolyForm Noncommercial 1.0.0 | **Settled** | Embedded at compile time via `include_str!` |
| `src-tauri/src/pipeline/allowlist.txt` | Microsoft Presidio (`presidio-analyzer/conf/default_recognizers.yaml`) | MIT (`LICENSE_allowlist.txt`, `THIRD_PARTY_NOTICES.md`) | **Settled** | Upstream MIT notice & full text included |
| `src-tauri/src/pipeline/LICENSE_allowlist.txt` | Microsoft Presidio | MIT | **Settled** | Licence text for `allowlist.txt` |
| `src-tauri/LICENSE_MPL-2.0.txt` | Mozilla Foundation | MPL 2.0 | **Settled** | Full MPL 2.0 text for `option-ext` / `fr_words.tsv` |
| `src-tauri/THIRD_PARTY_NOTICES.md` | Project-authored attribution index | — | **Settled** | Covers Lindera, IPADIC, Spanish Hunspell (`rla-es`), Presidio `allowlist.txt`, `option-ext`, PolyForm NC 1.0.0 code, and CC BY 4.0 data |

### Language Resource Data (`src-tauri/resources/<lang>/`)

#### Bangla (`bn`)

| File | Source | Licence (per `manifest.json` / `NOTICE_bn.md`) | Status | Notes |
|---|---|---|---|---|
| `bn/bn_words.tsv` | `bnwiktionary_titles`, `bnwiki_titles`, `hermitdave/FrequencyWords` (`bn_50k`) via OpenSubtitles/OPUS | CC BY-SA 4.0 (`LICENSE_bn_words_CC-BY-SA-4.0.txt`, `LICENSE_bn.txt`) | **Settled** | Cites `hermitdave/FrequencyWords` (`bn_50k`) under its `CC BY-SA 4.0` content licence |
| `bn/bn_names.tsv` | Split: `bnwiki_category` (`172` rows), `manual_verification` (`8` rows) | `CC BY-SA 4.0` (Wikipedia subset); `CC BY 4.0` (`manual_verification` rows) | **Settled** | Gemini-generated rows split into `bn_names_gemini.tsv` |
| `bn/bn_names_gemini.tsv` | Philosofile (Google Gemini, curated and pruned by Wasif Amir; `2,344` `gemini_unverified` rows) | `CC BY 4.0` (`LICENSE_CC-BY-4.0.txt`) | **Settled** | Split from `bn_names.tsv` under `CC BY 4.0` |
| `bn/bn_suffixes.tsv` | Universal Dependencies Bengali-BRU | CC BY-SA 4.0 | **Settled** | Third-party attribution recorded |
| `bn/bn_romanized.txt` | `bn_names_gemini.tsv` (`romanized` column; Google Gemini, curated and pruned by Wasif Amir) | `CC BY 4.0` (`LICENSE_CC-BY-4.0.txt`) | **Settled** | Curated and pruned AI-generated romanized names |
| `bn/manifest.json`, `bn/NOTICE_bn.md`, `bn/LICENSE_bn.txt`, `bn/LICENSE_bn_words_CC-BY-SA-4.0.txt`, `bn/LICENSE_CC-BY-4.0.txt` | Metadata & notices | — | **Settled** | Notices and full licence texts complete |

#### German (`de`)

| File | Source | Licence (per `manifest.json` / `NOTICE_de.md`) | Status | Notes |
|---|---|---|---|---|
| `de/de_names.txt` | Statistisches Bundesamt (Destatis) — *Vornamen und Familiennamen* | `dl-de/by-2-0` | **Settled** | Destatis `dl-de/by-2-0` attribution & project transliterations; header status `Settled` |
| `de/manifest.json`, `de/NOTICE_de.md` | Metadata & notice | — | **Settled** | Clean upstream licence metadata |

#### English (`en`)

| File | Source | Licence (per `manifest.json` / `NOTICE_en.md`) | Status | Notes |
|---|---|---|---|---|
| `en/en_words_full.txt` | NLTK `words` corpus (Unix `words`) + 41 project business keywords | Public Domain (NLTK corpus); `CC BY 4.0` (41 project additions) | **Settled** | Project additions documented in `NOTICE_en.md` |
| `en/en_words_unix.txt` | Unix `words` via NLTK `words` corpus | Public Domain | **Settled** | Public Domain |
| `en/en_names.txt` | NLTK `names` corpus (Kantrowitz v1.3) + `smashew/NameDatabases` + 7 overlapping project extras | Mark Kantrowitz Names Corpus License (`LICENSE_en_names.txt`) / The Unlicense / `CC BY 4.0` (project extras) | **Settled** | Contains `7` project-authored extras under `CC BY 4.0` |
| `en/manifest.json`, `en/NOTICE_en.md`, `en/LICENSE_en_names.txt` | Metadata & notices | — | **Settled** | Upstream notices preserved |

#### Spanish (`es`)

| File | Source | Licence (per `manifest.json` / `NOTICE_es.md`) | Status | Notes |
|---|---|---|---|---|
| `es/es_words.txt` | LibreOffice / Apache OpenOffice Spanish dictionary (`rla-es` by Santiago Bosio, v2.8 / `es_ES`) | MPL 1.1 (triple-licensed MPL 1.1 / LGPL 3.0+ / GPL 3.0+) | **Settled** | Covered in `NOTICE_es.md` and `src-tauri/THIRD_PARTY_NOTICES.md` |
| `es/es_names.txt` | Instituto Nacional de Estadistica (INE) + Wikidata | INE reuse terms (`Ley 37/2007` / `CC BY 4.0`) + Wikidata `CC0 1.0` | **Settled** | Header status `Settled`; INE/Wikidata notices recorded |
| `es/es_surnames_ine.csv` | Instituto Nacional de Estadistica (INE) (`2025`, 4,425 rows) | `CC BY 4.0` (INE) / `Ley 37/2007` | **Settled** | INE attribution/non-distortion terms documented (`LICENSE_es_names.txt` present) |
| `es/es_surnames_ai_supplement.csv` | Philosofile (Google Gemini, curated and pruned by Wasif Amir, 275 rows) | `CC BY 4.0` (`LICENSE_CC-BY-4.0.txt`) | **Settled** | Curated and pruned AI-generated surname supplement |
| `es/es_name_gated.txt` | Derived from `es_surnames_ine.csv`, `es_surnames_ai_supplement.csv`, and Leipzig Corpora (`spa_news_2024_1M`, `spa_web_2016_1M`, `spa_wikipedia_2021_1M`) | `CC BY 4.0` (`LICENSE_CC-BY-4.0.txt`, with INE / Leipzig / project attribution) | **Settled** | Dual INE/Leipzig & project-authored CC BY 4.0 attribution |
| `es/manifest.json`, `es/NOTICE_es.md`, `es/LICENSE_es_names.txt`, `es/LICENSE_CC-BY-4.0.txt` | Metadata & notices | — | **Settled** | Upstream notices & CC BY 4.0 text present |

#### French (`fr`)

| File | Source | Licence (per `manifest.json` / `NOTICE_fr.md`) | Status | Notes |
|---|---|---|---|---|
| `fr/fr_words.tsv` | Grammalecte Classique (`LibreOffice/dictionaries` `fr_FR`, v7.7) | MPL 2.0 | **Settled** | Attribution & license summary in `LICENSE_fr.txt`, `NOTICE_fr.md`, and full MPL 2.0 text in `src-tauri/LICENSE_MPL-2.0.txt` |
| `fr/fr_name_gated.txt` | Derived from `hermitdave/FrequencyWords` (`fr_50k`) vs INSEE `prenoms` | CC BY-SA 4.0 | **Settled** | Attributed to `hermitdave/FrequencyWords` (`fr_50k`, `CC BY-SA 4.0` content) / OpenSubtitles / INSEE |
| `fr/fr_names.txt` | INSEE Fichier des prenoms via `ThinkR-open/prenoms` (`nat2021`) | Licence Ouverte v2.0 (Etalab) | **Settled** | Header status `Settled`; Etalab Licence Ouverte v2.0 recorded |
| `fr/manifest.json`, `fr/NOTICE_fr.md`, `fr/LICENSE_fr.txt` | Metadata & notices | — | **Settled** | Heading in `NOTICE_fr.md` updated to `fr_names.txt` |

#### Hindi (`hi`)

| File | Source | Licence (per `manifest.json` / `NOTICE_hi.md`) | Status | Notes |
|---|---|---|---|---|
| `hi/hi_words.tsv.zst` | English Wiktionary Hindi Extract via `kaikki.org` (`2026-10-30`) | CC BY-SA 4.0 | **Settled** | `LICENSE_hi_dictionary.txt` (CC BY-SA 4.0 full text) included |
| `hi/hi_freq.tsv.zst` | Leipzig Corpora Collection (`hin_mixed_2019_1M`, `hin_news_2022_1M`, `hin_wikipedia_2021_1M`) | CC BY 4.0 | **Settled** | `LICENSE_hi_names.txt` (CC BY 4.0 full text) + LREC'12 attribution included |
| `hi/hi_names_male_clean.csv` | Philosofile (Gemini-generated, curated and pruned by Wasif Amir) | `CC BY 4.0` (`LICENSE_CC-BY-4.0.txt`, `LICENSE_hi_names.txt`) | **Settled** | Curated Gemini-generated list under `CC BY 4.0` |
| `hi/hi_names_female_clean.csv` | Philosofile (Gemini-generated, curated and pruned by Wasif Amir) | `CC BY 4.0` (`LICENSE_CC-BY-4.0.txt`, `LICENSE_hi_names.txt`) | **Settled** | Curated Gemini-generated list under `CC BY 4.0` |
| `hi/hi_surnames_clean.csv` | Philosofile (Gemini-generated, curated and pruned by Wasif Amir) | `CC BY 4.0` (`LICENSE_CC-BY-4.0.txt`, `LICENSE_hi_names.txt`) | **Settled** | Curated Gemini-generated list under `CC BY 4.0` |
| `hi/manifest.json`, `hi/NOTICE_hi.md`, `hi/LICENSE_hi_dictionary.txt`, `hi/LICENSE_hi_names.txt`, `hi/LICENSE_CC-BY-4.0.txt` | Metadata & notices | — | **Settled** | Notices and full CC BY 4.0 / CC BY-SA 4.0 texts complete |

#### Italian (`it`)

| File | Source | Licence (per `manifest.json` / `NOTICE_it.md`) | Status | Notes |
|---|---|---|---|---|
| `it/it_names.txt` | Istituto Nazionale di Statistica (ISTAT) — *I nomi e cognomi più diffusi* | IODL 2.0 | **Settled** | Header status `Settled` |
| `it/manifest.json`, `it/NOTICE_it.md` | Metadata & notice | — | **Settled** | Clean upstream metadata |

#### Japanese (`ja`)

| File | Source | Licence (per `manifest.json` / `NOTICE_ja.md`) | Status | Notes |
|---|---|---|---|---|
| `ja/ja_names.txt` | Wikidata (`ja_names_joined.tsv`) | CC0 1.0 | **Settled** | Wikidata CC0 1.0 |
| `ja/ja_surnames.txt` | Wikidata (`a7955fa3-query_4.csv`) | CC0 1.0 | **Settled** | Wikidata CC0 1.0 |
| `ja/ja_given_romanized.txt` | Wikidata (`en`) + project-generated kana Hepburn/passport expansions | `CC0 1.0` (Wikidata base) / `CC BY 4.0` (project-generated kana expansions) | **Settled** | Project-generated expansions licensed under `CC BY 4.0` |
| `ja/ja_surnames_romanized.txt` | Wikidata (`en`) + project-generated ASCII/diacritic alias expansions | `CC0 1.0` (Wikidata base) / `CC BY 4.0` (project-generated ASCII alias expansions) | **Settled** | Project-generated expansions licensed under `CC BY 4.0` |
| `ja/ipadic/*.zst` (9 files: `char_def.bin.zst`, `dict.trie.zst`, `dict.vals.zst`, `dict.valsidx.zst`, `dict.words.zst`, `dict.wordsidx.zst`, `matrix.mtx.zst`, `metadata.json.zst`, `unk.bin.zst`) | `mecab-ipadic` (v`2.7.0-20070801`) via Lindera | IPADIC Notice (NAIST / ICOT Free Software) | **Settled** | Full `NOTICE_ipadic.md` and `THIRD_PARTY_NOTICES.md` included |
| `ja/manifest.json`, `ja/NOTICE_ja.md`, `ja/NOTICE_ipadic.md` | Metadata & notices | — | **Settled** | Notices complete |

#### Korean (`ko`)

| File | Source | Licence (per `manifest.json` / `NOTICE_ko.md`) | Status | Notes |
|---|---|---|---|---|
| `ko/ko_names.txt` | Wikidata (`CC0 1.0`) + `7` project-authored additions | `CC0 1.0` (Wikidata); `CC BY 4.0` (project-authored additions) | **Settled** | Project-authored additions licensed under `CC BY 4.0` |
| `ko/ko_surnames.txt` | Wikidata (`CC0 1.0`) + `5` project-authored additions | `CC0 1.0` (Wikidata); `CC BY 4.0` (project-authored additions) | **Settled** | Project-authored additions licensed under `CC BY 4.0` |
| `ko/ko_names_romanized.txt` | Wikidata (`581fb123-1794212598643_query_3.csv`) + project-authored filtering & alias expansions | `CC0 1.0` (Wikidata base); `CC BY 4.0` (project-authored filtering & ASCII alias expansions) | **Settled** | Project-authored expansions licensed under `CC BY 4.0` |
| `ko/manifest.json`, `ko/NOTICE_ko.md` | Metadata & notice | — | **Settled** | Accurately documents split Wikidata CC0 + CC BY 4.0 project-authored terms |

#### Dutch (`nl`)

| File | Source | Licence (per `manifest.json` / `NOTICE_nl.md`) | Status | Notes |
|---|---|---|---|---|
| `nl/nl_names.txt` | Sociale Verzekeringsbank (SVB) / CBS | CC0 1.0 / Open Overheid | **Settled** | Header status `Settled` |
| `nl/manifest.json`, `nl/NOTICE_nl.md` | Metadata & notice | — | **Settled** | Clean upstream metadata |

#### Portuguese (`pt`)

| File | Source | Licence (per `manifest.json` / `NOTICE_pt.md`) | Status | Notes |
|---|---|---|---|---|
| `pt/pt_BR_words.tsv` | VERO (Verificador Ortográfico Livre) `pt_BR` v3.2.15 | LGPLv3 | **Settled** | Full `LICENSE_pt_BR.txt` included |
| `pt/pt_PT_words.tsv` | Hunspell `pt_PT` Dictionary (Projecto Natura, v20251001) | MPL 1.1 (elected from GPLv2 / LGPLv2.1 / MPL 1.1) | **Settled** | Full `LICENSE_pt_PT.txt` included |
| `pt/pt_name_gated.txt` | Leipzig Corpora vs Portuguese name lists | `CC BY 4.0` (`manifest.json`, `NOTICE_pt.md`) | **Settled** | Derived from Leipzig (`CC BY 4.0`) and `pt_names.txt` (`CC BY 4.0`) |
| `pt/pt_names.txt` | Philosofile (Google Gemini, curated and pruned by Wasif Amir) | `CC BY 4.0` (`LICENSE_CC-BY-4.0.txt`) | **Settled** | Curated Gemini-generated list under `CC BY 4.0` |
| `pt/freq/pt_freq.tsv.zst` | Leipzig Corpora Collection (`por-pt_web_2015_1M`, `por-br_newscrawl_2011_1M`, `por_news_2024_1M`) | CC BY 4.0 | **Settled** | LREC'12 attribution in `NOTICE_pt.md` (note: read only by `#[cfg(test)]` unit test, not runtime scrubbing) |
| `pt/manifest.json`, `pt/NOTICE_pt.md`, `pt/LICENSE_pt_BR.txt`, `pt/LICENSE_pt_PT.txt`, `pt/LICENSE_CC-BY-4.0.txt` | Metadata & notices | — | **Settled** | Notices and full licence texts complete |
