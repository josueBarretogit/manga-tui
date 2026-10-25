---
name: download-tester
description: >-
  Use this skill to test, validate, and debug manga chapter downloads across supported
  formats (CBZ, EPUB, PDF, and RAW images) in manga-tui. Covers running cargo make
  tasks, inspecting generated test_results, and verifying archive structures.
---

# Manga Download Tester Skill

This skill provides procedures for running, debugging, and validating chapter downloader tests in `manga-tui`.

---

## 1. Supported Formats & Architecture

Downloader implementations live under `src/backend/manga_downloader/`:
- `cbz_downloader.rs`: ZIP container containing sorted images and `.cbz` extension.
- `epub_downloader.rs`: Valid EPUB container with XML container, OPF manifest, and XHTML image wrappers.
- `pdf_downloader.rs`: Single PDF document embedding rendered images.
- `raw_images.rs`: Chapter folder containing individual image files (`1.jpg`, `2.jpg`, etc.).

Test outputs are generated into the directory `./test_results/`.

---

## 2. Running Download Tests

Tests in `manga_downloader` are marked `#[ignore]` because they perform file I/O operations and write files to disk.

### Option A: Using `cargo-make` (Recommended)
```powershell
# Test all download formats at once:
cargo make download-all

# Or test a single format:
cargo make download cbz
cargo make download epub
cargo make download pdf
cargo make download raw_images
```

### Option B: Using `cargo test` directly
```powershell
# Run all downloader tests:
cargo test backend::manga_downloader -- --ignored --nocapture

# Run a specific downloader test:
cargo test backend::manga_downloader::cbz_downloader -- --ignored --nocapture
cargo test backend::manga_downloader::epub_downloader -- --ignored --nocapture
cargo test backend::manga_downloader::pdf_downloader -- --ignored --nocapture
cargo test backend::manga_downloader::raw_images -- --ignored --nocapture
```

---

## 3. Validating Download Outputs

Inspect `./test_results/` after running tests:

1. **Verify File Existence & Non-Zero Size**:
   ```powershell
   Get-ChildItem -Path "./test_results" -Recurse | Select-Object Name, Length, FullName
   ```

2. **CBZ / EPUB Validation**:
   - Ensure the archive is not corrupted. Can be tested by opening with zip reader utilities or tar.
   - For EPUB: verify the first entry is `mimetype` with `application/epub+zip` and uncompressed.

3. **PDF Validation**:
   - Verify file starts with `%PDF-`.

4. **Raw Images Validation**:
   - Ensure files are named sequentially (`1.jpg`, `2.jpg`, ...) and have non-zero byte size.

---

## 4. Troubleshooting Common Download Errors

- **Permission Denied / Path Too Long**: Windows has MAX_PATH limitations. Ensure manga and chapter titles are sanitized using `ChapterToDownloadSanitized` before path concatenation.
- **Image Decoding Failure**: Corrupted image bytes from mocks in `data_test/images/` or unsupported WebP/AVIF formats.
