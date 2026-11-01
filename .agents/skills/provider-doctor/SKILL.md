---
name: provider-doctor
description: >-
  Use this skill to diagnose, debug, and repair broken manga providers in manga-tui.
  Covers investigating failing searches, chapter list errors, image loading issues,
  inspecting live HTML/API responses, fixing CSS selectors in response.rs, and
  bypassing HTTP 403/Cloudflare blocks.
---

# Provider Doctor Skill

This skill provides a standardized diagnostic and repair workflow when a manga provider in `manga-tui` stops working or yields errors.

---

## 1. Triage & Reproduction

### Step 1.1: Run Provider Live Tests
Locate the provider's test suite in `src/backend/manga_provider/<provider>.rs` and execute:
```powershell
cargo test backend::manga_provider::<provider> -- --ignored --nocapture
```

If no live test exists for the failing behavior, write a temporary `#[tokio::test] #[ignore]` test in `<provider>.rs` targeting the failing method.

### Step 1.2: Classify the Failure
Identify which phase is breaking:
| Failure Type | Typical Symptoms | Suspected Component |
| :--- | :--- | :--- |
| **Search Broken** | Returns 0 results or `NoneError` | `search_mangas` / `response.rs` search selector |
| **Manga Details Broken** | Cannot open manga page / 404 | `get_manga_by_id` / slug URL format |
| **Chapters Missing** | Manga opens but chapter list is empty | `get_all_chapters` / chapter table selector |
| **Pages Blank / Reader Stuck** | Reader shows 0 pages | `get_chapter_pages` / image selector or base64 decoding |
| **HTTP 403 Forbidden** | `reqwest::Error` 403 / Cloudflare challenge | Missing `Referer`, `User-Agent`, or IP block |

---

## 2. Inspecting Live Response

When a CSS selector or JSON parser fails, inspect the actual live response returned by the site.

### Step 2.1: Fetch HTML to Scratchpad
Fetch the HTML payload using `curl` with standard browser headers to inspect what the server is currently returning:
```powershell
curl -s -A "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36" -H "Referer: <BASE_URL>" "<TARGET_URL>" -o "scratch/debug_page.html"
```

### Step 2.2: Analyze the Payload
Check `scratch/debug_page.html`:
1. **Cloudflare Challenge?** Search for `Just a moment...` or `challenge-platform`. If present, headers need adjusting (see [references/common_blockers.md](./references/common_blockers.md)).
2. **Changed Class Names?** Websites often update CSS class names (e.g. Tailwind hashed classes like `.flex.gap-2.py-1`). Check if target elements moved to semantic tags or data attributes (e.g. `[data-chapter-id]`).
3. **Moved to Client-Side Hydration?** Search for `<script id="__NEXT_DATA__">` or `window.__INITIAL_STATE__`. Often sites stop rendering HTML in PHP/SSR and embed all data into a JSON script tag. Parsing this JSON is 10x more resilient than CSS scraping!

---

## 3. Repairing Parsers (`response.rs`)

### CSS Selector Repairs
Locate the failing selector in `src/backend/manga_provider/<provider>/response.rs`:
- Use robust selectors: prefer tag hierarchy, `id`, or `data-*` attributes over fragile utility classes.
- For example:
  ```rust
  // Fragile:
  Selector::parse("div.grid.grid-cols-2 > div.flex > a.text-blue-500").unwrap()

  // Resilient:
  Selector::parse("div[id*='chapter-list'] a[href*='/chapter/']").unwrap()
  ```

### Embedded JSON Extraction
If data moved to an embedded `<script>` tag:
```rust
let re = regex::Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
if let Some(caps) = re.captures(&html) {
    let json_str = &caps[1];
    let data: MyApiStructure = serde_json::from_str(json_str)?;
}
```

---

## 4. Fixing HTTP 403 / Image Loading Errors

If `get_raw_image` fails with 403 Forbidden:
1. Ensure the `REFERER` header is sent with the image request:
   ```rust
   let res = self.client.get(url)
       .header(http::header::REFERER, "<PROVIDER_BASE_URL>")
       .send()
       .await?;
   ```
2. Verify if a custom CDN hostname requires a specific `HOST` or `Accept: image/avif,image/webp,image/*,*/*;q=0.8` header.
3. Check if images are hosted on a different domain requiring different referer policies.

---

## 5. Verification & Installation

1. Re-run live test:
   ```powershell
   cargo test backend::manga_provider::<provider> -- --ignored --nocapture
   ```
2. Ensure no clippy regressions:
   ```powershell
   cargo clippy -- -D warnings
   ```
3. Run formatting:
   ```powershell
   cargo fmt --all
   ```
4. **Mandatory: Install updated binary globally** so the user can test the fixes immediately using `manga-tui`:
   ```powershell
   cargo install --path .
   ```

