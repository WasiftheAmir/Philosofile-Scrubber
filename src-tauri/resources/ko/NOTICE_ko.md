# Third-Party & Project-Authored Data Notice — Korean (`ko`)

This directory contains Korean given-name, surname, and romanized given-name lists loaded at runtime by Philosofile's PII scrubber.

All third-party entries across these lists come from **Wikidata** under **CC0 1.0 Universal** (`https://creativecommons.org/publicdomain/zero/1.0/`). In addition, 12 common Korean names and surnames were restored as project-authored additions (**"Added by the project maintainer as common Korean names. Not taken from a dataset."**, licence set with the project's release licence).

## 1. Korean Given Names (`ko_names.txt`, `5,816` entries)

- **Source:** Wikidata base (`query (3).csv`, `5,809` entries) + project-authored additions (`7` entries).
- **Licence:** **CC0 1.0 Universal** (`https://creativecommons.org/publicdomain/zero/1.0/`) for the Wikidata base; project-authored additions licence set with the project's release licence.
- **Project-authored additions (`7` given names):** Added by the project maintainer as common Korean names. Not taken from a dataset:
  - `민준`, `상훈`, `경호`, `미란`, `병헌`, `경옥`, `영애`

## 2. Korean Surnames (`ko_surnames.txt`, `597` entries)

- **Source:** Wikidata base (`query (3).csv`, `592` entries) + project-authored additions (`5` entries).
- **Licence:** **CC0 1.0 Universal** (`https://creativecommons.org/publicdomain/zero/1.0/`) for the Wikidata base; project-authored additions licence set with the project's release licence.
- **Project-authored additions (`5` surnames):** Added by the project maintainer as common Korean names. Not taken from a dataset:
  - `남궁`, `제갈`, `독고`, `서문`, `동방`

## 3. Romanized Korean Given Names (`ko_names_romanized.txt`, `6,113` entries)

This file combines upstream Wikidata data with project-authored filtering and alias expansions:

- **Source:** Wikidata (`581fb123-1794212598643_query_3.csv`), `en` label romanizations.
- **Licence:** **CC0 1.0 Universal** (Wikidata base, `https://creativecommons.org/publicdomain/zero/1.0/`) + project-authored filtering and ASCII alias expansions (governed by the project's release licence).
- **Version / Date:** `2026-11-09`
- **Project-Authored Changes:** Trimmed and filtered to entries with at least 1 ASCII letter and `>= 3` letters (excluding hyphens), lowercased, space-containing entries dropped, exact-deduplicated, sorted, and expanded with ASCII alias forms (stripping macrons `ā→a`, `ē→e`, `ī→i`, `ō→o`, `ū→u`, other Latin diacritics, and apostrophes).
