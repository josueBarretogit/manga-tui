<h1 align="center">
<strong>Manga-tui </strong>
</h1>

<h3 align="center">
    Terminal manga reader and downloader
</h3>

<p align="center">

https://github.com/user-attachments/assets/7f7f61e7-058f-4d3e-a6d4-ac692cc51bc8
    
</p>




## Table of contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration](#configuration)

## Features

- Read manga in your terminal (Wezterm, iterm2, Kitty)
[gif showing reader page]
- Advanced search (with filters)
[gif showing how filters work]
- Reading history is stored locally (with no login required)
[gif showing feed page]
- Download manga
[gif showing download]

## Installation

### Using cargo

```shell
cargo install manga-tui
```


## Usage

After installation run the binary

```shell
manga-tui
```

Manga and reading history is stored in the `manga-tui` directory, to know where it is run: 


```shell
manga-tui --data-dir 

# or

manga-tui -d
```

On linux it will output something like: `~/.local/share/manga-tui`
If you want to change the location you can set the environment variable `MANGA_TUI_DATA_DIR` to some path pointing to a directory, example: /home/user/somedirectory 

