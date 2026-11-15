# Portuguese Language Resources Attribution & Notice

This directory and the Portuguese PII scrubber pipeline incorporate data and rules from the following open sources. Note that the Philosofile scrubber component is planned to be open source.

## 1. Brazilian Portuguese Dictionary (`pt_BR_words.tsv`)
- **Upstream Source**: **VERO (Verificador Ortográfico Livre) pt_BR v3.2.15** by Raimundo Santos Moura and the Brazilian community.
- **Distribution File**: `VeroptBR3215AOC.oxt`
- **Version / Date**: Version 3.2.15 (Release date: 2021-08-03; dictionary build: 2020-11-24)
- **Checksums**: MD5: `ddfe6249452b2bfd1fe4cd95b3b0120e`
- **Download Source**: `https://web.archive.org/web/20260219135127/https://pt-br.libreoffice.org/assets/Uploads/PT-BR-Documents/VERO/VeroptBR3215AOC.oxt` (original distribution: `https://pt-br.libreoffice.org/assets/Uploads/PT-BR-Documents/VERO/VeroptBR3215AOC.oxt`)
- **Project URL**: `https://pt-br.libreoffice.org/projetos/projeto-vero-verificador-ortografico/`
- **Licence**: GNU Lesser General Public License version 3 (LGPLv3).
- **Verbatim Licence Notice from `pt_BR.aff`**:
  ```text
  #   VERO - Verificador Ortográfico Livre - Versão 3.2
  #   Copyright (C) 2006 - 2013  por Raimundo Santos Moura
  #   <raimundo.smoura@gmail.com>
  #   Brasil - outubro 2013
  #   Este é um dicionário para correção ortográfica da língua Portuguesa
  #   para o Hunspell.
  #   Este programa é livre e pode ser redistribuído e/ou modificado nos
  #   termos Licença Pública Geral Menor versão 3 (LGPLv3), como publicado
  #   pela Free Software Foundation e pela Mozilla Public License como
  #   publicado pela Mozilla Foundation.
  ```
- **Verbatim Licence Notice from `README_pt_BR.txt`**:
  ```text
  pt-BR: Este dicionário está em desenvolvimento por Raimundo Moura e sua equipe.
  Ele está licenciado sob os termos da Licença Pública Geral Menor versão 3 (LGPLv3),
  como publicado pela Free Software Foundation e pela Mozilla Public License como
  publicado pela Mozilla Foundation . Os créditos estão disponíveis em
  http://pt-br.libreoffice.org/projetos/projeto-vero-verificador-ortografico/
  ```
- **Content & Processing**: Contains 438,238 entries (pt_BR base dictionary stems and Hunspell affix inflections confirmed in the Leipzig corpus, plus unconfirmed base stems unconditionally preserved).

## 2. European Portuguese Dictionary (`pt_PT_words.tsv`)
- **Upstream Source**: **Hunspell pt_PT Dictionary (v20251001)** by José João de Almeida, Rui Vilela, Alberto Simões (Universidade do Minho, Projecto Natura).
- **Distribution File**: `hunspell-pt_PT-latest.tar.gz` (release directory `hunspell-pt_PT-20251001/`)
- **Version / Date**: Version 20251001 (Release date: 2025-10-01)
- **Checksums**:
  - MD5: `942f595ae6505f04234c5148cdd54c8d`
  - SHA-256: `2066157087e83264484a6d564e1d85258fe2c5eddb9713a42d6f6cf06b0a2ed9`
- **Download Source**: `https://natura.di.uminho.pt/download/sources/Dictionaries/hunspell/LATEST/hunspell-pt_PT-latest.tar.gz`
- **Licence**: Mozilla Public License Version 1.1 (MPL 1.1).
  The upstream package is tri-licensed under GPL v2.0 / LGPL v2.1 / MPL 1.1:
  ```text
  All dictionary files and associated programs are currently covered by
  the (GPL/LGPL/MPL), by this order. See also COPYING file for more
  details, if available.

  Regarding license versions: 
       1. GPL Version 2
       2. LGPL Version 2.1
       3. MPL Version 1.1
  ```
  Under this tri-licensing structure, Philosofile elects the MPL 1.1 option for `pt_PT_words.tsv`.
- **Content & Processing**: Contains 131,586 entries (pt_PT base dictionary stems and Hunspell affix inflections confirmed in the Leipzig corpus, plus unconfirmed base stems unconditionally preserved).

## 3. Portuguese Gated Words (`pt_name_gated.txt`)
- **Source**: Derived at build time using the build-time subtraction rule ($r = \log_{10}(\text{name\_count} / \text{word\_freq}) \ge -1.0$) comparing Leipzig corpus frequencies against the Portuguese name lists.
- **Content**: A flat list of lowercase words (5,292 words) that are both common dictionary entries and names, gated by capitalization (redacted when capitalized mid-sentence, protected as dictionary words when lowercase).
- **License**: Creative Commons Attribution 4.0 International (CC BY 4.0).

## 4. Portuguese Frequency Table (`pt_freq.tsv.zst`)
- **Source**: Leipzig Corpora Collection (Universität Leipzig, https://wortschatz.uni-leipzig.de/en/download/Portuguese)
- **Corpora Used**:
  - `por-pt_web_2015_1M` (Portugal Web 2015, 1 Million sentences)
  - `por-br_newscrawl_2011_1M` (Brazil Newscrawl 2011, 1 Million sentences)
  - `por_news_2024_1M` (Portuguese News 2024, 1 Million sentences)
- **License**: Creative Commons Attribution 4.0 International (CC BY 4.0)
- **Attribution**: D. Goldhahn, T. Eckart & U. Quasthoff: Building Large Monolingual Dictionaries at the Leipzig Corpora Collection: From 100 to 200 Languages. In: Proceedings of the 8th Language Resources and Evaluation Conference (LREC'12), 2012.
- **Content**: Compressed TSV mapping lowercase words to frequency counts (446,656 word-count pairs). Contains strictly word and count pairs, without any sentences, examples, or corpus text.

## 5. Portuguese Name Lists (`pt_names.txt`)
- **Origin & Licence**: Generated with Google Gemini, then curated and pruned by Wasif Amir. Licensed under CC BY 4.0. Changes: curated and pruned from the model's raw output.
- **Full Licence Text**: [`LICENSE_CC-BY-4.0.txt`](LICENSE_CC-BY-4.0.txt) (`https://creativecommons.org/licenses/by/4.0/`).
- **Third-Party Status**: Generated with Google Gemini (`male_first_names.csv`, `female_first_names.csv`, `surnames.csv`) with no third-party corpus source.
- **Processing**: Normalized to Unicode NFC and trimmed of whitespace. Combined into a flat, lowercase, deduplicated list of 11,628 unique names.

## 6. Particle Handling in Surnames
- **Connector Particles**: Connectors inside Portuguese surnames (`de`, `da`, `do`, `dos`, `das`, `e`) are handled during name token bridging (e.g., "João da Silva", "Maria de Lurdes", "Pedro e Sousa", "António dos Santos", "Ana das Dores", "Carlos do Carmo").

