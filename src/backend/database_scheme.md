# Manga-tui sqlite database

# app_version

This table is used to keep track of what version the user has installed on their machines, maybe this will be useful for future updates idk 

- version
    - type: TEXT PRIMARY KEY 


# history_types

The types of history, which are : `ReadingHistory` and `PlanToRead`

- id 
    - type: INTEGER PRIMARY KEY AUTOINCREMENT 
- name 
    - type: TEXT NOT NULL UNIQUE 

# mangas

To store which mangas the user is reading        

- id
    - type : TEXT  PRIMARY KEY
- title
    - type: TEXT  NOT NULL,
- created_at
    - type: DATETIME DEFAULT (datetime('now'))
- updated_at
    - type: DATETIME DEFAULT (datetime('now'))
- last_read
    - type: DATETIME DEFAULT (datetime('now'))
- deleted_at
    - type: DATETIME NULL
- img_url
    - type: TEXT NULL

# chapters 

To know which chapters the user has read

- id
    - type: TEXT  PRIMARY KEY
- title
    - type: TEXT  NOT NULL
- manga_id
    - type: TEXT  NOT NULL
- is_read
    - type: BOOLEAN NOT NULL DEFAULT 0
- is_downloaded
    - type: BOOLEAN NOT NULL DEFAULT 0

FOREIGN KEY (manga_id) REFERENCES mangas (id)

# manga_history_union

To query mangas that are in reading history or plan to read

- manga_id
    - type: TEXT 
- type_id
    - type: INTEGER 

PRIMARY KEY (manga_id, type_id),
FOREIGN KEY (manga_id) REFERENCES mangas (id),
FOREIGN KEY (type_id) REFERENCES history_types (id)

# chapter_notification_dismissals

Stores chapter IDs whose new-chapter notifications the user dismissed, so those notifications are not shown again.

- manga_id
    - type: TEXT NOT NULL
- provider
    - type: TEXT NOT NULL
- chapter_id
    - type: TEXT NOT NULL

PRIMARY KEY (manga_id, provider, chapter_id)

# favorite_mangas

Stores user-favorited mangas across all providers.

- manga_id
    - type: TEXT NOT NULL
- provider
    - type: TEXT NOT NULL
- title
    - type: TEXT NOT NULL
- created_at
    - type: DATETIME DEFAULT (datetime('now'))

PRIMARY KEY (manga_id, provider)

# search_history

Stores recently searched queries for quick recall and autocompletion cycling.

- query
    - type: TEXT PRIMARY KEY
- searched_at
    - type: DATETIME DEFAULT (datetime('now'))

# SQLite Configuration & Pragmas

To ensure high-performance concurrent reads and writes without `SQLITE_BUSY` contention:
- `PRAGMA journal_mode = WAL`: Write-Ahead Logging allows simultaneous concurrent readers while a write is occurring.
- `PRAGMA busy_timeout = 5000`: 5-second automatic retry timeout for SQLite locks.
- `PRAGMA foreign_keys = ON`: Enforces relational constraints.
- `PRAGMA synchronous = NORMAL`: Balances safety and write performance in WAL mode.

# Performance Indexes

- `idx_chapters_manga_lookup`: `CREATE INDEX IF NOT EXISTS idx_chapters_manga_lookup ON chapters(manga_id, is_read, is_downloaded);`
  - Optimizes `check_chapters_read`, `save_download_status`, and history queries.
- `idx_mangas_provider_last_read`: `CREATE INDEX IF NOT EXISTS idx_mangas_provider_last_read ON mangas(provider, last_read DESC);`
  - Optimizes reading history and timeline lookups ordered by recent activity.
- `idx_favorite_mangas_provider`: `CREATE INDEX IF NOT EXISTS idx_favorite_mangas_provider ON favorite_mangas(provider);`
  - Speeds up favorite manga filtering per provider.


