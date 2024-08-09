<h1 align="center">
<strong> 📖 Manga-tui 🖥️ </strong>
</h1>

<h3 align="center">
    Terminal manga reader and downloader
</h3>

<div align="center">
    <img alt="top language" src="https://img.shields.io/github/languages/top/josuebarretogit/manga-tui">
    <a href="https://crates.io/crates/manga-tui">
        <img alt="crates io downloads" src="https://img.shields.io/crates/d/manga-tui?logo=rust&label=crates.io downloads">
    </a>
    <a href="https://github.com/josueBarretogit/manga-tui/blob/main/LICENSE">
        <img alt="License" src="https://img.shields.io/github/license/josueBarretogit/Manga-tui?style=flat-square&color=blue">
    </a>
</div >

<p align="center">

https://github.com/user-attachments/assets/2b693bd3-ec30-4d6e-bcc4-6cf457a860b1

</p>


## Table of contents

- [Features](#features)
- [Installation](#installation)
- [Image rendering](#image-rendering)
- [Usage](#usage)
- [Configuration](#configuration)
- [Motivation](#motivation)
- [Credits](#credits)

## Features


- Advanced search (with filters)

https://github.com/user-attachments/assets/c1e21aa1-8a51-4c47-baea-9f56dcd0d6a4

- Read manga in your terminal (Wezterm, iTerm2, or Kitty, any terminal that has support for graphics protocol) 
  
https://github.com/user-attachments/assets/70f321ff-13d1-4c4b-9c37-604271456ab2

- Reading history is stored locally (with no login required)

 https://github.com/user-attachments/assets/47e88e89-f73c-4575-9645-2abb80ca7d63

- Download manga

https://github.com/user-attachments/assets/64880a98-74c8-4656-8cf8-2c1daf5375d2


## Installation

### Using cargo

```shell
cargo install manga-tui
```

### AUR

You can install `manga-tui` from the [AUR](https://aur.archlinux.org/packages/manga-tui) with using an [AUR helper](https://wiki.archlinux.org/title/AUR_helpers).

```shell
paru -S manga-tui
```

## Binary release

Download a binary from the [releases page](https://github.com/josueBarretogit/manga-tui/releases/latest)

## Image rendering

Use a terminal that can render images such as Wezterm (Personally I recommend using this one It's the one used in the videos), iTerm2 or Kitty, <br />

For more information see : [image-support](https://github.com/benjajaja/ratatui-image?tab=readme-ov-file#compatibility-matrix)

No images will be displayed if the terminal does not have image support (but `manga-tui` will still work as a manga downloader)

## Usage

After installation run the binary

```shell
manga-tui
```

Manga downloads and reading history is stored in the `manga-tui` directory, to know where it is run: 


```shell
manga-tui --data-dir 

# or

manga-tui -d
```

On linux it will output something like: `~/.local/share/manga-tui` <br />

On the `manga-tui` directory there will be 3 directories
- `history`, which contains a sqlite database to store reading history
- `mangaDownloads`, where manga will be downloaded 
- `errorLogs`, for storing posible errors / bugs 

If you want to change the location you can set the environment variable `MANGA_TUI_DATA_DIR` to some path pointing to a directory, like: <br />

```shell
export MANGA_TUI_DATA_DIR="/home/user/Desktop/mangas"
```


## Configuration

By default `manga-tui` will search mangas in english, you can change the language by running:


```shell
# `es` corresponds to the Iso code for spanish
manga-tui lang --set 'es'
```

Check the available languages and their Iso codes by running:


```shell
manga-tui lang --print
```

## Motivation
I wanted to make a "How linux user does ..." but for manga, [here is the video](https://www.youtube.com/watch?v=K0FsGRqEc1c) also this is a great excuse to start reading manga again 

## Credits

Many thanks to Mangadex for providing the free API please consider supporting them ❤️  <br />
Many thanks to the [Ratatui organization](https://github.com/ratatui-org) for making such a good library for making TUI's in rust 🐭 <br />
Many thanks to the developer of the [Ratatui-image crate](https://crates.io/crates/ratatui-image) for providing a widget that renders images in the terminal 🖼️ <br />

Consider giving a star to this project ⭐
