# Provider Registration Checklist

When adding `<name>` (struct `<Name>Provider`, enum variant `MangaProviders::<Name>`), you must touch the following 7 files:

---

### 1. `src/backend/manga_provider.rs`
* Declare module:
  ```rust
  pub mod <name>;
  ```
* Add to enum `MangaProviders`:
  ```rust
  pub enum MangaProviders {
      // ...
      #[strum(to_string = "<name>")]
      <Name>,
  }
  ```

---

### 2. `src/backend/multi_provider.rs`
* Import:
  ```rust
  use crate::backend::manga_provider::<name>::<Name>Provider;
  ```
* Add field to `MultiProvider`:
  ```rust
  pub struct MultiProvider {
      // ...
      pub <name>: <Name>Provider,
  }
  ```
* Initialize in `MultiProvider::new`:
  ```rust
  <name>: <Name>Provider::new(Arc::clone(&cache)),
  ```
* Set provider enum in `search_all`:
  ```rust
  manga.provider = Some(MangaProviders::<Name>);
  ```
* Match arm in dispatch methods:
  - `get_manga_by_id`: `MangaProviders::<Name> => pool.<name>.get_manga_by_id(manga_id)...`
  - `get_chapter_pages`: `MangaProviders::<Name> => pool.<name>.get_chapter_pages_url_with_extension(...)`
  - `get_raw_image`: `MangaProviders::<Name> => pool.<name>.get_raw_image(...)`
  - `get_manga_chapters`: `MangaProviders::<Name> => pool.<name>.get_all_chapters(...)`
  - `get_popular_mangas`: `MangaProviders::<Name> => pool.<name>.get_popular_mangas(...)`
  - `get_latest_mangas`: `MangaProviders::<Name> => pool.<name>.get_latest_mangas(...)`

---

### 3. `src/main.rs`
* Import:
  ```rust
  use backend::manga_provider::<name>::<Name>Provider;
  ```
* Match arm in provider router:
  ```rust
  MangaProviders::<Name> => {
      run_app(
          terminal,
          events,
          <Name>Provider::new(cache_provider),
          // ...
      ).await
  }
  ```

---

### 4. `src/view/app.rs`
* Specify default page size:
  ```rust
  MangaProviders::<Name> => 20, // Or whatever the site uses
  ```

---

### 5. `src/view/widgets/search.rs`
* Search item provider tag style and prefix:
  ```rust
  MangaProviders::<Name> => ("<Name>", Color::<ChosenColor>),
  ```
  ```rust
  MangaProviders::<Name> => ("[<Name>] ", Color::<ChosenColor>),
  ```

---

### 6. `src/view/pages/manga.rs`
* Manga details header provider badge:
  ```rust
  crate::backend::manga_provider::MangaProviders::<Name> => ("<Name>", ratatui::style::Color::<ChosenColor>),
  ```

---

### 7. Verification
Run check to ensure all exhaustive pattern matches compile:
```powershell
cargo check
```
