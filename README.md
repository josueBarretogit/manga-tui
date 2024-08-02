<h1 align="center">
<strong>Manga-tui </strong>
</h1>

<h3 align="center">
    Terminal manga reader and downloader
</h3>

<p align="center">

    

https://github.com/user-attachments/assets/2b693bd3-ec30-4d6e-bcc4-6cf457a860b1


</p>




## Table of contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration](#configuration)

## Features

- Read manga in your terminal (If it supports graphical protocols like : Wezterm, iterm2, Kitty)
  
https://github.com/user-attachments/assets/70f321ff-13d1-4c4b-9c37-604271456ab2


- Advanced search (with filters)

https://github.com/user-attachments/assets/c1e21aa1-8a51-4c47-baea-9f56dcd0d6a4


- Reading history is stored locally (with no login required)
- https://github.com/user-attachments/assets/47e88e89-f73c-4575-9645-2abb80ca7d63

- Download manga


https://github.com/user-attachments/assets/64880a98-74c8-4656-8cf8-2c1daf5375d2


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

