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
