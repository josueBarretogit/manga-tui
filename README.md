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
    <a href="https://github.com/josueBarretogit/manga-tui/releases/latest">
        <img alt="downloads" src="https://img.shields.io/github/downloads/josuebarretogit/manga-tui/total">
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

- Download manga (available formats: cbz, epub and raw images) 

https://github.com/user-attachments/assets/ba785668-7cf1-4367-93f9-6e6e1f72c12c

- Download all chapters of a manga (available formats: cbz, epub and raw images) 

https://github.com/user-attachments/assets/26ad493f-633c-41fc-9d09-49b316118923


## Installation

### Using cargo

```shell
cargo install manga-tui --locked
```

### AUR

You can install `manga-tui` from the [AUR](https://aur.archlinux.org/packages/manga-tui) with using an [AUR helper](https://wiki.archlinux.org/title/AUR_helpers).

```shell
paru -S manga-tui
```

### Nix

If you have the [Nix package manager](https://nixos.org/), this repo provides a flake that builds the latest git version from source.

Simply run the following:

```sh
nix run 'github:josueBarretogit/manga-tui'
```

Or, to install persistently:

```sh
nix profile install 'github:josueBarretogit/manga-tui'
```

## Binary release

Download a binary from the [releases page](https://github.com/josueBarretogit/manga-tui/releases/latest)

## Image rendering

Use a terminal that can render images such as Wezterm (Personally I recommend using this one It's the one used in the videos), iTerm2 or Kitty <br />

For more information see : [image-support](https://github.com/benjajaja/ratatui-image?tab=readme-ov-file#compatibility-matrix)

No images will be displayed if the terminal does not have image support (but `manga-tui` will still work as a manga downloader)

## Usage

After installation just run the binary

```shell
manga-tui
```


## Configuration


Manga downloads and reading history is stored in the `manga-tui` directory, to know where it is run: 


```shell
manga-tui --data-dir 

# or

manga-tui -d
```

On linux it will output something like: `~/.local/share/manga-tui` <br />

On the `manga-tui` directory there will be 4 directories
- `history`, which contains a sqlite database to store reading history
- `config`, which contains a TOML file where you can define download format and image quality
- `mangaDownloads`, where manga will be downloaded 
- `errorLogs`, for storing posible errors / bugs 

If you want to change the location of this directory you can set the environment variable `MANGA_TUI_DATA_DIR` to some path pointing to a directory, like: <br />

```shell
export MANGA_TUI_DATA_DIR="/home/user/Desktop/mangas"
```

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
