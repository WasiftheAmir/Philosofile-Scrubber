# Known Issues

1. **Short unlabelled numeric codes (`scrub_catch_all` 6-digit threshold)**
   - The `scrub_catch_all` rule uses a 6-digit minimum threshold outside high-sensitivity documents and misses short unlabelled numeric strings (for example, `12345` in `TEST 12345`).
   - Label-based coverage (`SHORT_CODE` for `CVC`, `CVV`, `CVV2`, `CSC`, `PIN`, `OTP`, `one-time code`, `verification code`, `passcode`, `security code`, `access code`, plus localized `PIN` labels) is in place.
   - Threshold adjustment for unlabelled short numbers is **under review**. `image_redact::tests::test_synthetic_text_12345_ocr_blur` is marked `#[ignore]` pending that decision.

2. **Spanish `"Mancha"` over-redaction**
   - In `"Don Quijote de la Mancha recorrió los caminos de España."`, `"de la Mancha"` is currently redacted to `"Don [NAME] recorrió los caminos de España."` because `"Mancha"` is name-leaning under the Spanish frequency ratio rule (`log10(444/477) = -0.03`). Tracked as a known issue.

3. **Japanese harness leak rate**
   - The Japanese evaluation harness currently has an approximately `15%` leak rate on the test set (known).

4. **Hindi documented miss `hi_04`**
   - Documented recall miss on test case `hi_04` (known).

5. **Windows OCR and face-detection paths untested on macOS**
   - The Windows WinRT OCR (`Windows.Media.Ocr`) and face-detection (`Windows.Media.FaceAnalysis`) code paths in `image_redact.rs` compile behind `#[cfg(target_os = "windows")]` and are untested when building/testing on macOS.
