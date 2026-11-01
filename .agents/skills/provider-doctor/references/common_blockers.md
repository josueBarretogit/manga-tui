# Common Scraper Blockers & Solutions in manga-tui

This reference details the most frequent causes of provider breakage and established workarounds.

---

### 1. HTTP 403 Forbidden on Images or Pages

#### Cause: Missing Referer Header
Many CDNs verify the `Referer` to block hotlinking.
* **Fix**: In `GetRawImage::get_raw_image` or client headers, always set:
  ```rust
  .header(http::header::REFERER, BASE_URL)
  ```
  *(Note: some sites require `https://<site-name>/` with trailing slash, while others require the specific manga or chapter URL as Referer).*

#### Cause: Cloudflare Browser Integrity Check
Cloudflare flags requests missing common desktop headers.
* **Fix**: Ensure default client headers contain:
  ```rust
  default_headers.insert(
      USER_AGENT,
      HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36")
  );
  default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9,es;q=0.8"));
  default_headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"));
  default_headers.insert("sec-ch-ua", HeaderValue::from_static("\"Not(A:Brand\";v=\"99\", \"Google Chrome\";v=\"133\", \"Chromium\";v=\"133\""));
  default_headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
  default_headers.insert("sec-ch-ua-platform", HeaderValue::from_static("\"Windows\""));
  ```

---

### 2. Chapters List Sorted Inverted or Missing Numbers

#### Cause: Site Changed Chapter Number Formatting
Sites often use formats like `"Ch. 10.5"` or `"Capítulo 10 - Sub Español"`.
* **Fix**: Use regex parsing to isolate the floating-point chapter number:
  ```rust
  // Extract first floating-point or integer number in title
  let re = regex::Regex::new(r#"(?i)(?:ch(?:apter)?\.?|cap(?:ítulo)?\.?)\s*(\d+(?:\.\d+)?)"#).unwrap();
  ```
* **Verify**: Ensure both `number: f64` and `number_str: String` are populated properly so that chapter navigation (`get_next_chapter`, `get_previous_chapter`) works in the reader.

---

### 3. Infinite Scroll / Dynamic Lazy-Loaded Chapters

#### Cause: Chapters Loaded via Hidden AJAX Endpoint
Sites often show only the first 20 chapters in initial HTML and load the rest via an API (e.g. `/api/manga/{id}/chapters` or `/ajax/chapters?manga_id=...`).
* **Fix**: Look in browser developer tools (Network tab) or search HTML for AJAX endpoints. It is almost always cleaner and faster to fetch the JSON or partial HTML from that AJAX endpoint directly rather than scraping the initial page.

---

### 4. CSRF Tokens & Session Cookies

#### Cause: Search endpoint requires CSRF protection
Some sites (like MangaOni) require a CSRF token passed with POST requests.
* **Fix**:
  1. Enable `cookie_store(true)` on `reqwest::Client`.
  2. Fetch homepage first to grab the token from `<meta name="csrf-token" content="...">`.
  3. Include the token in search headers: `.header("X-CSRF-TOKEN", token)`.
