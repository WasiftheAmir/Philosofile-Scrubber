# French Language Resources Attribution & Notice

This directory and the French PII scrubber pipeline incorporate data and rules from the following open sources:

## 1. French Word List (`fr_words.tsv`)
- **Source**: Grammalecte French Orthographic Dictionary ("Classique" v7.7) by Olivier R. (https://grammalecte.net/)
- **Repository**: LibreOffice/dictionaries (`fr_FR/dictionaries/fr.dic` and `fr.aff`) on GitHub
- **URL**: https://raw.githubusercontent.com/LibreOffice/dictionaries/master/fr_FR/dictionaries/
- **License**: Mozilla Public License Version 2.0 (MPL 2.0)
- **License URL**: https://www.mozilla.org/MPL/2.0/
- **Access Date**: 2026-11-10
- **Processing**: Expanded affix rules into a flat, lowercase, deduplicated list of 690,814 words.

## 2. French Gated Words (`fr_name_gated.txt`)
- **Source**: Derived at build time from the Hermit Dave `FrequencyWords` (fr_50k) frequency comparison against INSEE names.
- **Content**: A flat list of words only (1,601 lowercase words). No counts, no frequencies, and no protected-word lists.
- **License**: Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0), attributed to Hermit Dave / OpenSubtitles / INSEE.
- **License URL**: https://creativecommons.org/licenses/by-sa/4.0/
- **Access Date**: 2026-11-10

## 3. French Given Names (`src-tauri/src/pipeline/names/fr.txt`)
- **Source**: Institut National de la Statistique et des Études Économiques (INSEE) - Fichier des prénoms (1900–2021)
- **Repository**: ThinkR-open/prenoms on GitHub (`data-raw/nat2021.csv`)
- **URL**: https://raw.githubusercontent.com/ThinkR-open/prenoms/master/data-raw/nat2021.csv
- **INSEE Reference**: https://www.insee.fr/fr/statistiques/2540004
- **License**: Licence Ouverte v2.0 (Etalab)
- **License URL**: https://www.etalab.gouv.fr/licence-ouverte-open-licence/
- **Access Date**: 2026-11-10
- **Processing**: Excluded `_PRENOMS_RARES`. Aggregated counts across all birth years and sexes. Filtered to names with count $\ge 100$ (13,464 unique names). Accents preserved.

## 4. Frequency Comparison (Build Time Only)
- **Source**: Hermit Dave `FrequencyWords` (French 50k list)
- **Repository**: hermitdave/FrequencyWords on GitHub (`content/2018/fr/fr_50k.txt`)
- **URL**: https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/fr/fr_50k.txt
- **License**: MIT (code) / Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0, content derived from OpenSubtitles)
- **Access Date**: 2026-11-10
- **Usage**: Used strictly in memory during dictionary build-time comparison ($r = \log_{10}(\text{name\_count} / \text{word\_freq\_50k}) \ge -1.0$) to produce `fr_name_gated.txt`. Frequency counts and the top-1,000 protection list are never bundled, shipped, or committed.

## 5. Known Scrubber Limitations
- **Surname Particles**: Particles inside French surnames (such as `de`, `la`, `du`, `d'`, `des`) in running text remain unredacted by design unless captured by an explicit form-field label or name salutation block.

