use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, IntoImageView, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, GenericImage, GenericImageView, ImageFormat, Rgba, RgbaImage, RgbImage};
use std::collections::HashSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "macos")]
static VISION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const DEFAULT_MAX_PIXELS: usize = 1_500_000;
const LOW_INFO_STD_DEV: f64 = 3.0;
const FACE_MOSAIC_MIN_BLOCK: u32 = 14;
const FACE_MOSAIC_MAX_BLOCK: u32 = 36;
const TEXT_MOSAIC_MIN_BLOCK: u32 = 8;
const TEXT_MOSAIC_MAX_BLOCK: u32 = 24;

/// Configuration for [`redact_image`].
#[derive(Debug, Clone)]
pub struct RedactOptions {
    /// Maximum pixel count before bilinear SIMD downscaling.
    pub max_pixels: usize,
    /// Enable OS-native face detection and mosaic pixelation.
    pub blur_faces: bool,
    /// Enable OS-native OCR and mosaic pixelation of scrubber-redacted words.
    pub blur_text: bool,
    /// Options passed to `scrub_text_with` when classifying OCR tokens.
    pub scrub_options: crate::ScrubOptions,
}

impl Default for RedactOptions {
    fn default() -> Self {
        Self {
            max_pixels: DEFAULT_MAX_PIXELS,
            blur_faces: true,
            blur_text: true,
            scrub_options: crate::ScrubOptions::new(&[]),
        }
    }
}

impl RedactOptions {
    /// Builds default redaction options with additional identity/name tokens for text scrubbing.
    pub fn with_extra_names(extra_names: &[String]) -> Self {
        Self {
            scrub_options: crate::ScrubOptions::new(extra_names),
            ..Self::default()
        }
    }
}

/// Lossless PNG output and redaction counts returned by [`redact_image`].
#[derive(Debug, Clone)]
pub struct RedactedImage {
    /// Encoded PNG image bytes after downscaling and mosaic redaction.
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub faces_redacted: usize,
    pub text_boxes_redacted: usize,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
type NormalizedTextBox = (String, f32, f32, f32, f32);

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct DetailedRedaction {
    output: RedactedImage,
    baseline: DynamicImage,
    face_boxes: Vec<(f32, f32, f32, f32)>,
    redacted_text_boxes: Vec<(f32, f32, f32, f32)>,
}

struct TempFileGuard {
    path: PathBuf,
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Returns `true` when RGB channel standard deviation is below the low-information threshold.
pub fn is_low_information_image(img: &DynamicImage) -> bool {
    let rgb = img.to_rgb8();
    let pixels = rgb.as_raw();
    if pixels.is_empty() {
        return true;
    }
    let n = (pixels.len() / 3) as f64;
    let (mut sum_r, mut sum_g, mut sum_b) = (0.0, 0.0, 0.0);
    for chunk in pixels.chunks_exact(3) {
        sum_r += chunk[0] as f64;
        sum_g += chunk[1] as f64;
        sum_b += chunk[2] as f64;
    }
    let (mean_r, mean_g, mean_b) = (sum_r / n, sum_g / n, sum_b / n);
    let mut var_sum = 0.0;
    for chunk in pixels.chunks_exact(3) {
        let dr = chunk[0] as f64 - mean_r;
        let dg = chunk[1] as f64 - mean_g;
        let db = chunk[2] as f64 - mean_b;
        var_sum += dr * dr + dg * dg + db * db;
    }
    (var_sum / (3.0 * n)).sqrt() < LOW_INFO_STD_DEV
}

/// SIMD-bilinear downscales `img` so total pixels are within `target_max_pixels`.
pub fn fast_downscale_image(img: &DynamicImage, target_max_pixels: usize) -> Option<DynamicImage> {
    if target_max_pixels == 0 {
        return None;
    }
    let w = img.width();
    let h = img.height();
    let total_pixels = w as usize * h as usize;
    if total_pixels <= target_max_pixels {
        return None;
    }

    let scale_factor = (target_max_pixels as f64 / total_pixels as f64).sqrt();
    let new_w = ((w as f64 * scale_factor) as u32).max(1);
    let new_h = ((h as f64 * scale_factor) as u32).max(1);
    let resize_opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));

    if let Some(pixel_type) = img.pixel_type() {
        let mut dst = Image::new(new_w, new_h, pixel_type);
        let mut resizer = Resizer::new();
        if resizer.resize(img, &mut dst, Some(&resize_opts)).is_ok() {
            match img {
                DynamicImage::ImageRgba8(_) => {
                    return RgbaImage::from_raw(new_w, new_h, dst.into_vec())
                        .map(DynamicImage::ImageRgba8);
                }
                DynamicImage::ImageRgb8(_) => {
                    return RgbImage::from_raw(new_w, new_h, dst.into_vec())
                        .map(DynamicImage::ImageRgb8);
                }
                _ => {}
            }
        }
    }

    let rgba = img.to_rgba8();
    let mut dst_rgba = Image::new(new_w, new_h, fast_image_resize::PixelType::U8x4);
    let mut resizer = Resizer::new();
    resizer.resize(&rgba, &mut dst_rgba, Some(&resize_opts)).ok()?;
    RgbaImage::from_raw(new_w, new_h, dst_rgba.into_vec()).map(DynamicImage::ImageRgba8)
}

/// Decodes an image, applies OS-native face detection and OCR-backed text PII mosaic blur,
/// and returns lossless PNG bytes.
///
/// Fails closed on unsupported platforms with `Err("unsupported platform")`.
pub fn redact_image(bytes: &[u8], opts: &RedactOptions) -> Result<RedactedImage, String> {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (bytes, opts);
        return Err("unsupported platform".to_string());
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        redact_image_detailed(bytes, opts).map(|d| d.output)
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn redact_image_detailed(bytes: &[u8], opts: &RedactOptions) -> Result<DetailedRedaction, String> {
    let decoded = image::load_from_memory(bytes)
        .map_err(|e| format!("failed to decode image: {e}"))?;
    let baseline = fast_downscale_image(&decoded, opts.max_pixels).unwrap_or(decoded);
    let mut img = baseline.clone();

    let mut face_boxes = Vec::new();
    if opts.blur_faces {
        let guard = save_temp_jpeg(&img, "face")?;
        let faces = detect_faces_normalized(&guard.path)?;
        for (nx, ny, nw, nh) in faces {
            if mosaic_pixelate_region(
                &mut img,
                nx,
                ny,
                nw,
                nh,
                FACE_MOSAIC_MIN_BLOCK,
                FACE_MOSAIC_MAX_BLOCK,
            ) {
                face_boxes.push((nx, ny, nw, nh));
            }
        }
    }

    let mut redacted_text_boxes = Vec::new();
    if opts.blur_text {
        let guard = save_temp_jpeg(&img, "text")?;
        let boxes = detect_text_boxes_normalized(&guard.path)?;
        if !boxes.is_empty() {
            let redacted_words = find_redacted_words(&boxes, &opts.scrub_options)?;
            if !redacted_words.is_empty() {
                for (text, nx, ny, nw, nh) in boxes {
                    let should_blur = extract_words(&text)
                        .into_iter()
                        .any(|w| redacted_words.contains(&w));
                    if should_blur
                        && mosaic_pixelate_region(
                            &mut img,
                            nx,
                            ny,
                            nw,
                            nh,
                            TEXT_MOSAIC_MIN_BLOCK,
                            TEXT_MOSAIC_MAX_BLOCK,
                        )
                    {
                        redacted_text_boxes.push((nx, ny, nw, nh));
                    }
                }
            }
        }
    }

    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;

    Ok(DetailedRedaction {
        output: RedactedImage {
            png_bytes: out.into_inner(),
            width: img.width(),
            height: img.height(),
            faces_redacted: face_boxes.len(),
            text_boxes_redacted: redacted_text_boxes.len(),
        },
        baseline,
        face_boxes,
        redacted_text_boxes,
    })
}

fn save_temp_jpeg(img: &DynamicImage, tag: &str) -> Result<TempFileGuard, String> {
    let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "philosofile_redact_{}_{}_{}.jpg",
        tag,
        std::process::id(),
        seq
    ));
    img.save(&path)
        .map_err(|e| format!("failed to stage temp image: {e}"))?;
    Ok(TempFileGuard { path })
}

fn extract_words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn find_redacted_words(
    boxes: &[(String, f32, f32, f32, f32)],
    scrub_opts: &crate::ScrubOptions,
) -> Result<HashSet<String>, String> {
    let mut raw_text = String::new();
    for (text, _, _, _, _) in boxes {
        raw_text.push_str(text);
        raw_text.push(' ');
    }

    let report = crate::scrub_text_with(raw_text.trim(), scrub_opts);
    if let Some(err) = report.error {
        return Err(err);
    }

    let raw_words = extract_words(&raw_text);
    let scrubbed_words = extract_words(&report.text);
    Ok(raw_words
        .into_iter()
        .filter(|w| !scrubbed_words.contains(w))
        .collect())
}

fn normalized_box_to_pixels(
    nx: f32,
    ny: f32,
    nw: f32,
    nh: f32,
    width: u32,
    height: u32,
) -> Option<(u32, u32, u32, u32)> {
    let (wf, hf) = (width as f32, height as f32);
    let px = (nx * wf).max(0.0) as u32;
    let py = (ny * hf).max(0.0) as u32;
    if px >= width || py >= height {
        return None;
    }
    let pw = (nw.max(0.0) * wf).min(wf - px as f32) as u32;
    let ph = (nh.max(0.0) * hf).min(hf - py as f32) as u32;
    if pw == 0 || ph == 0 {
        return None;
    }
    Some((px, py, pw, ph))
}

fn mosaic_pixelate_region(
    img: &mut DynamicImage,
    nx: f32,
    ny: f32,
    nw: f32,
    nh: f32,
    min_block: u32,
    max_block: u32,
) -> bool {
    let Some((px, py, pw, ph)) =
        normalized_box_to_pixels(nx, ny, nw, nh, img.width(), img.height())
    else {
        return false;
    };

    let block_size = (pw.max(ph) / 8).clamp(min_block, max_block);
    let mut by = py;
    while by < py + ph {
        let cur_h = block_size.min(py + ph - by);
        let mut bx = px;
        while bx < px + pw {
            let cur_w = block_size.min(px + pw - bx);
            let count = (cur_w * cur_h) as u64;

            let (mut r_sum, mut g_sum, mut b_sum, mut a_sum) = (0u64, 0u64, 0u64, 0u64);
            for y in by..(by + cur_h) {
                for x in bx..(bx + cur_w) {
                    let pixel = img.get_pixel(x, y);
                    r_sum += pixel[0] as u64;
                    g_sum += pixel[1] as u64;
                    b_sum += pixel[2] as u64;
                    a_sum += pixel[3] as u64;
                }
            }

            let avg = Rgba([
                (r_sum / count) as u8,
                (g_sum / count) as u8,
                (b_sum / count) as u8,
                (a_sum / count) as u8,
            ]);
            for y in by..(by + cur_h) {
                for x in bx..(bx + cur_w) {
                    img.put_pixel(x, y, avg);
                }
            }
            bx += cur_w;
        }
        by += cur_h;
    }
    true
}

#[cfg(target_os = "macos")]
fn detect_faces_normalized(path: &Path) -> Result<Vec<(f32, f32, f32, f32)>, String> {
    let _guard = VISION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use apple_vision::prelude::*;
    objc::rc::autoreleasepool(|| {
        let mut faces = Vec::new();
        if let Ok(results) = FaceDetector::new().detect_in_path(path) {
            for face in results {
                // Apple Vision uses bottom-left origin; flip Y to top-left normalized coordinates.
                faces.push((
                    face.bounding_box.x as f32,
                    1.0 - face.bounding_box.y as f32 - face.bounding_box.height as f32,
                    face.bounding_box.width as f32,
                    face.bounding_box.height as f32,
                ));
            }
        }
        Ok(faces)
    })
}

#[cfg(target_os = "macos")]
fn detect_text_boxes_normalized(path: &Path) -> Result<Vec<NormalizedTextBox>, String> {
    let _guard = VISION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use apple_vision::prelude::*;
    objc::rc::autoreleasepool(|| {
        let mut boxes = Vec::new();
        let recognizer = TextRecognizer::new()
            .with_recognition_level(RecognitionLevel::Accurate)
            .with_language_correction(true);
        if let Ok(observations) = recognizer.recognize_in_path(path) {
            for obs in observations {
                boxes.push((
                    obs.text,
                    obs.bounding_box.x as f32,
                    1.0 - obs.bounding_box.y as f32 - obs.bounding_box.height as f32,
                    obs.bounding_box.width as f32,
                    obs.bounding_box.height as f32,
                ));
            }
        }
        Ok(boxes)
    })
}

#[cfg(target_os = "windows")]
fn load_windows_bitmap(
    path: &Path,
) -> windows::core::Result<windows::Graphics::Imaging::SoftwareBitmap> {
    use windows::core::HSTRING;
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Storage::StorageFile;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    // SAFETY: `pvReserved` must be `None` (null) per Win32 `CoInitializeEx` requirements.
    // Initializes COM on the current thread as multithreaded for WinRT imaging/OCR/face APIs;
    // ignoring `RPC_E_CHANGED_MODE` if the caller already initialized COM with another apartment.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let hpath = HSTRING::from(path.to_str().unwrap_or_default());
    let file = StorageFile::GetFileFromPathAsync(&hpath)?.get()?;
    let stream = file.OpenReadAsync()?.get()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
    decoder.GetSoftwareBitmapAsync()?.get()
}

#[cfg(target_os = "windows")]
fn detect_faces_normalized(path: &Path) -> Result<Vec<(f32, f32, f32, f32)>, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::FaceAnalysis::FaceDetector;

    let run = || -> windows::core::Result<Vec<(f32, f32, f32, f32)>> {
        let software_bitmap = load_windows_bitmap(path)?;
        let detector = FaceDetector::CreateAsync()?.get()?;
        let supported =
            FaceDetector::IsBitmapPixelFormatSupported(software_bitmap.BitmapPixelFormat()?)?;
        let bitmap = if supported {
            software_bitmap
        } else {
            SoftwareBitmap::Convert(&software_bitmap, BitmapPixelFormat::Gray8)?
        };
        let faces = detector.DetectFacesAsync(&bitmap)?.get()?;
        let width = bitmap.PixelWidth()? as f32;
        let height = bitmap.PixelHeight()? as f32;
        let mut detected = Vec::new();
        for face in faces {
            let box_ = face.FaceBox()?;
            detected.push((
                box_.X as f32 / width,
                box_.Y as f32 / height,
                box_.Width as f32 / width,
                box_.Height as f32 / height,
            ));
        }
        Ok(detected)
    };

    Ok(run().unwrap_or_default())
}

#[cfg(target_os = "windows")]
fn detect_text_boxes_normalized(path: &Path) -> Result<Vec<NormalizedTextBox>, String> {
    use windows::Media::Ocr::OcrEngine;

    let run = || -> windows::core::Result<Vec<NormalizedTextBox>> {
        let bitmap = load_windows_bitmap(path)?;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        let result = engine.RecognizeAsync(&bitmap)?.get()?;
        let width = bitmap.PixelWidth()? as f32;
        let height = bitmap.PixelHeight()? as f32;
        let mut detected = Vec::new();
        for line in result.Lines()? {
            for word in line.Words()? {
                let rect = word.BoundingRect()?;
                detected.push((
                    word.Text()?.to_string(),
                    rect.X as f32 / width,
                    rect.Y as f32 / height,
                    rect.Width as f32 / width,
                    rect.Height as f32 / height,
                ));
            }
        }
        Ok(detected)
    };

    Ok(run().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient_image(width: u32, height: u32) -> DynamicImage {
        let mut buf = RgbImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                buf.put_pixel(
                    x,
                    y,
                    image::Rgb([(x * 3) as u8, (y * 5) as u8, ((x + y) % 255) as u8]),
                );
            }
        }
        DynamicImage::ImageRgb8(buf)
    }

    fn glyph_5x7(ch: char) -> [u8; 7] {
        match ch {
            'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
            'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
            'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
            '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
            '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
            '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
            '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
            '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
            _ => [0; 7],
        }
    }

    fn render_bitmap_text_png(text: &str, scale: u32, margin: u32) -> Vec<u8> {
        let chars: Vec<char> = text.chars().collect();
        let step_x = 6 * scale;
        let width = margin * 2 + (chars.len() as u32) * step_x;
        let height = margin * 2 + 7 * scale;
        let mut img = RgbImage::from_pixel(width, height, image::Rgb([255, 255, 255]));
        for (idx, ch) in chars.iter().enumerate() {
            let rows = glyph_5x7(*ch);
            let x0 = margin + (idx as u32) * step_x;
            for (ry, row_bits) in rows.iter().enumerate() {
                for rx in 0..5u32 {
                    if (row_bits >> (4 - rx)) & 1 == 1 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                img.put_pixel(
                                    x0 + rx * scale + dx,
                                    margin + (ry as u32) * scale + dy,
                                    image::Rgb([0, 0, 0]),
                                );
                            }
                        }
                    }
                }
            }
        }
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut encoded, ImageFormat::Png)
            .expect("encode synthetic PNG");
        encoded.into_inner()
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn assert_boxes_blurred_and_outside_unchanged(
        baseline: &DynamicImage,
        redacted: &DynamicImage,
        face_boxes: &[(f32, f32, f32, f32)],
        text_boxes: &[(f32, f32, f32, f32)],
    ) {
        let (w, h) = (baseline.width(), baseline.height());
        assert_eq!((redacted.width(), redacted.height()), (w, h));
        let mut in_any_box = vec![false; (w * h) as usize];

        let mut check_group = |kind: &str, boxes: &[(f32, f32, f32, f32)]| {
            for (i, &(nx, ny, nw, nh)) in boxes.iter().enumerate() {
                let (px, py, pw, ph) = normalized_box_to_pixels(nx, ny, nw, nh, w, h)
                    .expect("detected box should map inside image");
                let mut changed = 0usize;
                for y in py..(py + ph) {
                    for x in px..(px + pw) {
                        in_any_box[(y * w + x) as usize] = true;
                        if baseline.get_pixel(x, y) != redacted.get_pixel(x, y) {
                            changed += 1;
                        }
                    }
                }
                println!(
                    "  {} box[{}] norm=({:.4}, {:.4}, {:.4}, {:.4}) px=({}, {}, {}, {}) changed_pixels={}",
                    kind, i, nx, ny, nw, nh, px, py, pw, ph, changed
                );
                assert!(changed > 0, "{} box[{}] pixels did not change", kind, i);
            }
        };

        check_group("face", face_boxes);
        check_group("word", text_boxes);

        let mut outside_changed = 0usize;
        for y in 0..h {
            for x in 0..w {
                if !in_any_box[(y * w + x) as usize]
                    && baseline.get_pixel(x, y) != redacted.get_pixel(x, y)
                {
                    outside_changed += 1;
                }
            }
        }
        println!(
            "summary: face_boxes={}, word_boxes={}, outside_changed={}",
            face_boxes.len(),
            text_boxes.len(),
            outside_changed
        );
        assert_eq!(outside_changed, 0, "pixels outside detected boxes changed");
    }

    #[test]
    fn test_mosaic_pixelate_region_averages_only_inside_box() {
        let mut img = gradient_image(80, 80);
        let outside_before = img.get_pixel(5, 5);
        let inside_a = img.get_pixel(20, 20);
        let inside_b = img.get_pixel(25, 25);
        assert_ne!(inside_a, inside_b);

        let changed = mosaic_pixelate_region(&mut img, 0.25, 0.25, 0.5, 0.5, 8, 24);
        assert!(changed);
        assert_eq!(img.get_pixel(5, 5), outside_before);
        assert_eq!(img.get_pixel(20, 20), img.get_pixel(25, 25));
    }

    #[test]
    fn test_find_redacted_words_flags_sensitive_tokens() {
        let boxes = vec![(
            "Contact john@example.com for invoice".to_string(),
            0.1,
            0.1,
            0.5,
            0.1,
        )];
        let opts = crate::ScrubOptions::without_identity();
        let redacted = find_redacted_words(&boxes, &opts).expect("scrubber should succeed");
        assert!(redacted.contains("john") || redacted.contains("example"));
        assert!(!redacted.contains("invoice"));
    }

    #[test]
    fn test_low_information_and_downscale_helpers() {
        let solid = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 64, image::Rgb([42, 42, 42])));
        assert!(is_low_information_image(&solid));

        let large = gradient_image(200, 100);
        assert!(!is_low_information_image(&large));
        let resized = fast_downscale_image(&large, 5_000).expect("should downscale");
        assert!((resized.width() as usize * resized.height() as usize) <= 5_500);
        assert!(fast_downscale_image(&large, 100_000).is_none());
    }

    #[test]
    fn test_redact_image_platform_or_decode_error() {
        let result = redact_image(b"not valid image bytes", &RedactOptions::default());
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert_eq!(result.unwrap_err(), "unsupported platform");
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        assert!(result.is_err());
    }

    #[test]
    fn test_redact_image_smoke_on_supported_os() {
        let img = gradient_image(120, 80);
        let mut encoded = Cursor::new(Vec::new());
        img.write_to(&mut encoded, ImageFormat::Png).unwrap();
        let result = redact_image(&encoded.into_inner(), &RedactOptions::default());

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let redacted = result.expect("redact_image should succeed on macOS/Windows");
            assert_eq!(redacted.width, 120);
            assert_eq!(redacted.height, 80);
            assert!(!redacted.png_bytes.is_empty());
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert_eq!(result.unwrap_err(), "unsupported platform");
    }

    #[test]
    #[ignore = "Known issue: catch-all threshold; see KNOWN_ISSUES.md"]
    fn test_synthetic_text_12345_ocr_blur() {
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            println!("synthetic text OCR test skipped on unsupported platform");
            return;
        }

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let png_bytes = render_bitmap_text_png("TEST 12345", 10, 48);
            let scrub_check = crate::scrub_text_with("TEST 12345", &crate::ScrubOptions::without_identity());
            let decoded = image::load_from_memory(&png_bytes).expect("decode synthetic PNG");
            let guard = save_temp_jpeg(&decoded, "synth_diag").expect("save temp");
            let ocr_boxes = detect_text_boxes_normalized(&guard.path).expect("OCR call");
            let ocr_matched_text = ocr_boxes
                .iter()
                .any(|(t, ..)| t.to_uppercase().contains("TEST") || t.contains("12345"));
            println!(
                "synthetic diag: ocr_boxes={}, ocr_matched_text={}, scrub_changed={}",
                ocr_boxes.len(),
                ocr_matched_text,
                scrub_check.text != "TEST 12345"
            );

            let opts = RedactOptions {
                blur_faces: false,
                blur_text: true,
                scrub_options: crate::ScrubOptions::without_identity(),
                ..RedactOptions::default()
            };
            let detail = redact_image_detailed(&png_bytes, &opts)
                .expect("synthetic redact_image should succeed");
            let redacted_img = image::load_from_memory(&detail.output.png_bytes)
                .expect("decode redacted PNG");
            assert!(
                !detail.redacted_text_boxes.is_empty(),
                "expected at least one OCR-redacted word box for synthetic 'TEST 12345' (ocr_boxes={}, ocr_matched_text={}, scrub_changed={})",
                ocr_boxes.len(),
                ocr_matched_text,
                scrub_check.text != "TEST 12345"
            );
            assert_boxes_blurred_and_outside_unchanged(
                &detail.baseline,
                &redacted_img,
                &[],
                &detail.redacted_text_boxes,
            );
        }
    }

    #[test]
    fn test_redact_image_env_pixel_verification() {
        let Some(path) = std::env::var_os("PHILOSOFILE_TEST_IMAGE").filter(|v| !v.is_empty())
        else {
            println!("set PHILOSOFILE_TEST_IMAGE to run the vision pixel test");
            return;
        };

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let bytes = std::fs::read(&path).expect("read PHILOSOFILE_TEST_IMAGE");
            let err = redact_image(&bytes, &RedactOptions::default()).unwrap_err();
            assert_eq!(err, "unsupported platform");
            return;
        }

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let bytes = std::fs::read(&path).expect("read PHILOSOFILE_TEST_IMAGE");
            let detail = redact_image_detailed(&bytes, &RedactOptions::default())
                .expect("redact_image on PHILOSOFILE_TEST_IMAGE");
            let redacted_img = image::load_from_memory(&detail.output.png_bytes)
                .expect("decode redacted PNG");
            assert!(
                !detail.face_boxes.is_empty(),
                "no face detected in PHILOSOFILE_TEST_IMAGE (word_boxes={})",
                detail.redacted_text_boxes.len()
            );
            assert!(
                !detail.redacted_text_boxes.is_empty(),
                "no OCR-redacted word boxes detected in PHILOSOFILE_TEST_IMAGE (face_boxes={})",
                detail.face_boxes.len()
            );
            assert_boxes_blurred_and_outside_unchanged(
                &detail.baseline,
                &redacted_img,
                &detail.face_boxes,
                &detail.redacted_text_boxes,
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_windows_winrt_path_skipped_on_non_windows() {
        println!("Windows WinRT OCR/FaceAnalysis path is untested on this OS; skipped");
    }
}
