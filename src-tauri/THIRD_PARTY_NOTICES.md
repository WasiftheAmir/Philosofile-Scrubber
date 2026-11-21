# Third-Party Software Notices and Licenses

This project incorporates third-party software and data under the following licenses:

---

## 1. Lindera

- **Project**: Lindera (https://github.com/lindera/lindera)
- **Version**: 6.2.0
- **License**: MIT License

### MIT License Text

```
Copyright (c) 2019-2024 Lindera contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## 2. IPADIC (mecab-ipadic-2.7.0-20070801)

- **Source**: http://jaist.dl.sourceforge.net/project/mecab/mecab-ipadic/2.7.0-20070801/mecab-ipadic-2.7.0-20070801.tar.gz
- **Copyright Holders**: Nara Institute of Science and Technology (NAIST) and Institute for New Generation Computer Technology (ICOT)
- **Notice File**: Full verbatim copy of `lindera-ipadic/NOTICE.txt`:

```
===========================================================================
Lindera Morphological Analyzer
===========================================================================

This software includes a binary and/or source version of data from

  mecab-ipadic-2.7.0-20070801

which can be obtained from

  http://jaist.dl.sourceforge.net/project/mecab/mecab-ipadic/2.7.0-20070801/mecab-ipadic-2.7.0-20070801.tar.gz

===========================================================================
mecab-ipadic-2.7.0-20070801 Notice
===========================================================================

Nara Institute of Science and Technology (NAIST),
the copyright holders, disclaims all warranties with regard to this
software, including all implied warranties of merchantability and
fitness, in no event shall NAIST be liable for
any special, indirect or consequential damages or any damages
whatsoever resulting from loss of use, data or profits, whether in an
action of contract, negligence or other tortuous action, arising out
of or in connection with the use or performance of this software.

A large portion of the dictionary entries
originate from ICOT Free Software.  The following conditions for ICOT
Free Software applies to the current dictionary as well.

Each User may also freely distribute the Program, whether in its
original form or modified, to any third party or parties, PROVIDED
that the provisions of Section 3 ("NO WARRANTY") will ALWAYS appear
on, or be attached to, the Program, which is distributed substantially
in the same form as set out herein and that such intended
distribution, if actually made, will neither violate or otherwise
contravene any of the laws and regulations of the countries having
jurisdiction over the User or the intended distribution itself.

NO WARRANTY

The program was produced on an experimental basis in the course of the
research and development conducted during the project and is provided
to users as so produced on an experimental basis.  Accordingly, the
program is provided without any warranty whatsoever, whether express,
implied, statutory or otherwise.  The term "warranty" used herein
includes, but is not limited to, any warranty of the quality,
performance, merchantability and fitness for a particular purpose of
the program and the nonexistence of any infringement or violation of
any right of any third party.

Each user of the program will agree and understand, and be deemed to
have agreed and understood, that there is no warranty whatsoever for
the program and, accordingly, the entire risk arising from or
otherwise connected with the program is assumed by the user.

Therefore, neither ICOT, the copyright holder, or any other
organization that participated in or was otherwise related to the
development of the program and their respective officials, directors,
officers and other employees shall be held liable for any and all
damages, including, without limitation, general, special, incidental
and consequential damages, arising out of or otherwise in connection
with the use or inability to use the program or any product, material
or result produced or otherwise obtained by using the program,
regardless of whether they have been advised of, or otherwise had
knowledge of, the possibility of such damages at any time during the
project or thereafter.  Each user will be deemed to have agreed to the
foregoing by his or her commencement of use of the program.  The term
"use" as used herein includes, but is not limited to, the use,
modification, copying and distribution of the program and the
production of secondary products from the program.

In the case where the program, whether in its original form or
modified, was distributed or delivered to or received by a user from
any person, organization or entity other than ICOT, unless it makes or
grants independently of ICOT any specific warranty to the user in
writing, such person, organization or entity, will also be exempted
from and not be held liable to the user for any such damages as noted
above as far as the program is concerned.
```

---

## 3. Spanish Hunspell Dictionary (rla-es / LibreOffice)

- **Project**: Spanish spellchecker dictionary for LibreOffice / Apache OpenOffice (https://github.com/sbosio/rla-es)
- **Author**: Santiago Bosio and contributors
- **Version**: 2.8 / es_ES
- **License**: Mozilla Public License Version 1.1 (MPL-1.1) / GNU LGPL v3.0+ / GNU GPL v3.0+ (Bundled under MPL-1.1)

### MPL 1.1 Notice

The contents of `src-tauri/resources/es/es_words.txt` are derived from the Spanish spellchecker dictionary developed by Santiago Bosio, distributed under a disjunctive tri-license (GPLv3+, LGPLv3+, MPL-1.1). This project bundles the expanded word list under the terms of the Mozilla Public License, Version 1.1 (the "License"). You may obtain a copy of the License at https://www.mozilla.org/MPL/MPL-1.1.html.

## 4. Pipeline Allowlist (`src-tauri/src/pipeline/allowlist.txt`)

- **Source**: Microsoft Presidio (https://github.com/microsoft/presidio)
- **Copyright**: Portions Copyright (c) 2019 Microsoft Corporation
- **License**: MIT License — full license text at `src-tauri/src/pipeline/LICENSE_allowlist.txt`

---

## 5. `option-ext` (transitive Rust dependency via `dirs` / `dirs-sys`)

- **Crate**: `option-ext`
- **Version**: `0.2.0`
- **Source**: https://github.com/soc/option-ext
- **License**: Mozilla Public License Version 2.0 (`MPL-2.0`) — https://www.mozilla.org/en-US/MPL/2.0/
- **Full License Text**: [`src-tauri/LICENSE_MPL-2.0.txt`](LICENSE_MPL-2.0.txt)

---

## 6. Philosofile Scrubber Source Code & Pipeline Assets

- **Files**: `src-tauri/src/pipeline/scrubber.rs`, `src-tauri/src/pipeline/scrubber_tests.rs`, `src-tauri/src/pipeline/registry_check.rs`, `src-tauri/src/pipeline/image_redact.rs`, `src-tauri/src/pipeline/label_groups.tsv`, `src-tauri/src/pipeline/id_formats.csv`, crate manifest files (`Cargo.toml`, `Cargo.lock`), and root documentation (`README.md`, `INSTRUCTIONS.md`, `KNOWN_ISSUES.md`, `LICENCE_STATUS.md`)
- **Required Notice**: `Copyright (c) 2026 Wasif Amir (Philosofile scrubber)`
- **License**: `PolyForm-Noncommercial-1.0.0` (source-available, non-commercial use only; full text in root `LICENSE`)
- **Source URL**: https://polyformproject.org/licenses/noncommercial/1.0.0 (requested `https://polyformproject.org/licenses/noncommercial/1.0.0/`, canonical text served at `https://polyformproject.org/licenses/noncommercial/1.0.0`)
- **Fetch Date**: `2026-11-16`

---

## 7. Project-Authored & Gemini AI-Generated Data Lists (`CC BY 4.0`)

The following project-authored and curated Gemini AI-generated name/word supplements and romanization expansions are licensed under **Creative Commons Attribution 4.0 International (`CC-BY-4.0`)** (full text in `LICENSE_CC-BY-4.0.txt` in `bn/`, `es/`, `hi/`, `pt/`, and `https://creativecommons.org/licenses/by/4.0/`):

- **Bangla (`src-tauri/resources/bn/`)**: `bn_names_gemini.tsv` (`2,344` `gemini_unverified` rows split out from `bn_names.tsv`), `bn_romanized.txt`, and `manual_verification` rows (`8` entries) in `bn_names.tsv`. (`bn_words.tsv` — including `hermitdave/FrequencyWords` `bn_50k` under its `CC BY-SA 4.0` content license — `bn_suffixes.tsv`, and Wikipedia rows in `bn_names.tsv` remain under `CC BY-SA 4.0`.)
- **English (`src-tauri/resources/en/`)**: `41` project-authored business keywords in `en_words_full.txt` and `7` project extras in `en_names.txt`.
- **Spanish (`src-tauri/resources/es/`)**: `es_surnames_ai_supplement.csv` and `es_name_gated.txt` (derived from INE `CC BY 4.0`, Leipzig Corpora `CC BY 4.0`, and `es_surnames_ai_supplement.csv`).
- **Hindi (`src-tauri/resources/hi/`)**: `hi_names_male_clean.csv`, `hi_names_female_clean.csv`, `hi_surnames_clean.csv`.
- **Japanese (`src-tauri/resources/ja/`)**: Project-generated kana/ASCII alias expansions in `ja_given_romanized.txt` and `ja_surnames_romanized.txt` (combined with Wikidata `CC0 1.0` bases).
- **Korean (`src-tauri/resources/ko/`)**: Project-authored additions (`7` given names in `ko_names.txt`, `5` surnames in `ko_surnames.txt`) and filtering/ASCII alias expansions in `ko_names_romanized.txt` (combined with Wikidata `CC0 1.0` bases).
- **Portuguese (`src-tauri/resources/pt/`)**: `pt_names.txt`.

---

## 8. Third-Party Linguistic Datasets & License Separation

All bundled third-party linguistic datasets (`src-tauri/resources/<lang>/`) retain their respective upstream licenses (`CC0 1.0`, `CC BY 4.0`, `CC BY-SA 4.0`, `LGPLv3`, `MPL 1.1`, `MPL 2.0`, `dl-de/by-2-0`, `IODL 2.0`, `Licence Ouverte v2.0`, Public Domain, and IPADIC Notice), documented in root `LICENCE_STATUS.md` and each language directory's `NOTICE_*.md` and `manifest.json`. Share-alike and weak-copyleft licenses on individual third-party dataset files (e.g., `bn_words.tsv`, `bn_names.tsv` Wikipedia subset, `bn_suffixes.tsv`, `fr_name_gated.txt`, `hi_words.tsv.zst`, `es_words.txt`, `fr_words.tsv`, `pt_BR_words.tsv`, `pt_PT_words.tsv`) apply strictly to those individual dataset files as separate data aggregations loaded at runtime; the Rust scrubber source code itself is governed by the PolyForm Noncommercial License 1.0.0.

