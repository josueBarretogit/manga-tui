<h1 align="center">
<strong> 📖 Manga-tui 🖥️ </strong>
</h1>

<h3 align="center">
    Terminal-based manga reader and downloader written in rust 🦀
</h3>
<div align="center">
    <a href="https://github.com/josueBarretogit/manga-tui/actions/workflows/test.yml">
        <img alt="test passing" src="https://img.shields.io/github/actions/workflow/status/josueBarretogit/manga-tui/test.yml?label=tests">
    </a>
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
- [Dependencies](#Dependencies)
- [Installation](#installation)
- [Image rendering](#image-rendering)
- [Usage](#usage)
- [Manga providers](#manga-providers)
- [Configuration](#configuration)
- [Motivation](#motivation)
- [Credits](#credits)

## Features

- [Mangadex](https://mangadex.org/) and [Manganato](https://manganato.com/) are available as manga providers

- Track your reading history with [anilist integration](./docs/anilist.md) 

- Advanced search (with filters)

https://github.com/user-attachments/assets/c1e21aa1-8a51-4c47-baea-9f56dcd0d6a4

- Read manga in your terminal with terminals such as: Wezterm, iTerm2, Kitty, Ghostty 
  
https://github.com/user-attachments/assets/70f321ff-13d1-4c4b-9c37-604271456ab2

- Reading history is stored locally (with no login required)

 https://github.com/user-attachments/assets/47e88e89-f73c-4575-9645-2abb80ca7d63

- Download manga (available formats: cbz, epub and raw images) 

https://github.com/user-attachments/assets/ba785668-7cf1-4367-93f9-6e6e1f72c12c

- Download all chapters of a manga (available formats: cbz, epub and raw images) 

https://github.com/user-attachments/assets/26ad493f-633c-41fc-9d09-49b316118923


### Join the [discord](https://discord.gg/jNzuDCH3) server for further help, feature requests or to chat with contributors   

## Dependencies 

On linux you may need to install the D-bus secret service library

### Debian 
```shell
sudo apt install libdbus-1-dev pkg-config
```

### Fedora
```shell
sudo dnf install dbus-devel pkgconf-pkg-config
```

### Arch 
```shell
sudo pacman -S dbus pkgconf
```

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

Use a terminal that can render images such as [Wezterm](https://wezfurlong.org/wezterm/index.html) (Personally I recommend using this one It's the one used in the videos), [iTerm2](https://iterm2.com/), [Kitty](https://sw.kovidgoyal.net/kitty/) and [Ghostty](https://ghostty.org/download)  <br />
For more information see: [image-support](https://github.com/benjajaja/ratatui-image?tab=readme-ov-file#compatibility-matrix)

> [!WARNING]
> On windows image display is very buggy, see [this issue](https://github.com/josueBarretogit/manga-tui/issues/26) for more information

No images will be displayed if the terminal does not have image support  (but `manga-tui` will still work as a manga downloader)

## Usage

After installation just run the binary

```shell
manga-tui
```

## Manga providers

> [!WARNING]
> Expect any manga provider to fail at some point, either due to them closing operations due to a [lawsuit](https://www.japantimes.co.jp/news/2024/04/18/japan/crime-legal/manga-mura-copyright-ruling/) or the provider itself having issues on their end like [manganato](https://www.reddit.com/r/mangapiracy/comments/1iumo9v/mangakakalot_and_manganato_site_shutdown_mangabat/)

By default when you run `manga-tui` Mangadex will be used 

```shell
manga-tui
```
If you want to use manganato then run:

```shell
manga-tui -p manganato
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
- `config`, which contains the `manga-tui-config.toml` config file with the following fields:

```toml
# The format of the manga downloaded 
# values : cbz , raw, epub 
# default : cbz 
download_type = "cbz"

# Download image quality, low quality means images are compressed and is recommended for slow internet connections 
# values : low, high 
# default : low 
image_quality = "low"

# Pages around the currently selected page to try to prefetch
# values : 0-255
# default : 5
amount_pages = 5

# Whether or not bookmarking is done automatically, if false you decide which chapter to bookmark
# values : true, false
# default : true
auto_bookmark = true

# Whether or not downloading a manga counts as reading it on services like anilist
# values : true, false
# default : false
track_reading_when_download = false
```

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
