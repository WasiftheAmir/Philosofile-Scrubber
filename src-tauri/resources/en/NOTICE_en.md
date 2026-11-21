# Third-Party Data Notices — English (`en`)

This directory contains the English common-word lists and English/Latin name list loaded at runtime by Philosofile's PII scrubber.

## 1. `en_words_unix.txt` (`235,886` entries)

- **Source:** Unix `words` (`en`) dictionary distributed in NLTK Data `words.zip` (`nltk/nltk_data`, `packages/corpora/words.xml`).
- **Licence:** Public Domain (declared `public domain` in NLTK `packages/corpora/words.xml`).
- **Provenance:** Added to the repository in commit `9e1b79b` (2026-10-08) as `words/en` (see historical `words/README` referencing `http://en.wikipedia.org/wiki/Words_(Unix)`).

## 2. `en_words_full.txt` (`234,371` entries)

- **Source:** `nltk.corpus.words.words()` from NLTK Data `words.zip` (`packages/corpora/words.xml`), lowercased and filtered (`len(w) > 1 or w in ['a', 'i']`), unioned with 41 project-authored business/document keywords via `gen_dicts.py` / `generate_wordlist.py`.
- **Licence:** Public Domain (NLTK `words` corpus; 41 project-authored business/document keyword additions licensed under **CC BY 4.0**).
- **Provenance:** Added to the repository in commit `9e1b79b` (2026-10-08) as `wordlist_full.txt`.

## 3. `en_names.txt` (`6,679` entries)

- **Sources:**
  - **NLTK `names` corpus** (Mark Kantrowitz Names Corpus, Version 1.3, 1994-03-29; Copyright (C) 1991 Mark Kantrowitz, additions by Bill Ross) — originally `2,430` intersecting entries from `male.txt` / `female.txt`.
  - **`smashew/NameDatabases`** (`first names/all.txt` and `surnames/all.txt`) — originally `4,288` additional intersecting entries.
  - **Project extras** — 7 additional overlapping entries from 22 hardcoded extras in `gen_dicts.py`, intersected against `en_words_full.txt` and subsequently deduplicated/cleaned to `6,679` baseline entries.
- **Licences:**
  - NLTK `names` corpus: Copyright (C) 1991 Mark Kantrowitz; freely redistributable provided credit is given in published work and recipients receive a copy of `names/README`. See `LICENSE_en_names.txt`.
  - `smashew/NameDatabases`: Released under The Unlicense (public domain dedication). See `LICENSE_en_names.txt`.
  - Project extras: Licensed under **CC BY 4.0**.
- **Provenance Note:** Added in commit `9e1b79b` (2026-10-08) as `namelist.txt`. The `3,983` Wikidata/kana Japanese romanized given names previously appended in commit `e17f9f2` (2026-11-07) have been separated into `../ja/ja_given_romanized.txt`.
