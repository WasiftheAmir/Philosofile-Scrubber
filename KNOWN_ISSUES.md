# Known Issues

1. **Short unlabelled numeric codes (`scrub_catch_all` 6-digit threshold)**
   - The `scrub_catch_all` rule uses a 6-digit minimum threshold outside high-sensitivity documents and misses short unlabelled numeric strings (for example, `12345` in `TEST 12345`).
   - Label-based coverage (`SHORT_CODE` for `CVC`, `CVV`, `CVV2`, `CSC`, `PIN`, `OTP`, `one-time code`, `verification code`, `passcode`, `security code`, `access code`, plus localized `PIN` labels) is in place.
   - Threshold adjustment for unlabelled short numbers is **under review**. `image_redact::tests::test_synthetic_text_12345_ocr_blur` is marked `#[ignore]` pending that decision.

2. **Spanish `"Mancha"` over-redaction**
   - In `"Don Quijote de la Mancha recorrió los caminos de España."`, `"de la Mancha"` is currently redacted to `"Don [NAME] recorrió los caminos de España."` because `"Mancha"` is name-leaning under the Spanish frequency ratio rule (`log10(444/477) = -0.03`). Tracked as a known issue.

3. **Japanese evaluation recall (~15% leak rate)**
   - On the project's internal Japanese (`ja`) evaluation corpus (not bundled in this repository), the text scrubber currently exhibits an approximately `15%` residual PII leak rate. Improving recall on Japanese text without introducing false-positive redactions on common vocabulary remains an open area of work.

4. **Hindi evaluation recall miss**
   - On the project's internal Hindi (`hi`) evaluation corpus (not bundled in this repository), one benchmark fixture (internally tracked as `hi_04`) has a documented PII recall miss. Improving coverage for that edge case without regressing precision on common Devanagari words remains an open item.

5. **Windows OCR and face-detection paths untested on macOS**
   - The Windows WinRT OCR (`Windows.Media.Ocr`) and face-detection (`Windows.Media.FaceAnalysis`) code paths in `image_redact.rs` compile behind `#[cfg(target_os = "windows")]` and are untested when building/testing on macOS.

6. **ID-label same-line trailing sentence over-redaction (`baseline_issues::test_id_label_value_followed_by_ordinary_sentence_not_redacted`)**
   - When an ID-style label and value are followed by an ordinary sentence on the same line, the scrubber over-redacts trailing same-line sentence text beyond the ID value.
   - Rationale: Accepted trade-off — line-bounded label capture intentionally prioritizes recall on noisy or OCR-extracted forms over strict same-line token boundaries, which can consume trailing sentence text. Marked `#[ignore]` pending tuning against real OCR samples.
   - Status: accepted, under review.

7. **Name-label placeholder followed by capitalized phrase over-redaction (`baseline_issues::test_name_label_placeholder_followed_by_capitalized_phrase_over_redaction`)**
   - When a name-style label is followed by a redaction placeholder and a capitalized multi-word phrase on the same line, the scrubber over-redacts the capitalized phrase as part of the name.
   - Rationale: Accepted trade-off — line-bounded label capture treats subsequent capitalized same-line tokens after a name label as potential multi-part name continuations, prioritizing recall at the expense of over-redacting trailing capitalized phrases. Marked `#[ignore]` pending tuning against real OCR samples.
   - Status: accepted, under review.
