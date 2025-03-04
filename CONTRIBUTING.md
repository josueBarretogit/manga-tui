# Contribution Guide

Thank you for considering contributing. Please review the guidelines below before making a contribution.

## Reporting Issues

Before reporting, please check if an issue with the same content already exists.

### Reporting Bugs

When reporting a bug, please include the following information:

- Application version
- Terminal emulator and version being used
- Instructions to replicate the bug you found like : do x then y happens
- if posibble an error message found in the `manga-tui-error-logs.txt` file located where the `manga-tui` directory is, if you don't know the location of this directory run:


```shell
manga-tui --data-dir 

# or

manga-tui -d
```

### Setting up dev enviroment

Make sure you have installed [cargo make](https://github.com/sagiegurari/cargo-make)

After cloning the repository run:
```shell
cargo make
```
It will run all the ci workflow which consists of formatting, checking, building and testing the code (which includes both normal test and ignored tests)
after it is done a directory called `./test_results` will be created which is where the download tests produce their output

To run only the download test:
```shell
cargo make download-all
```

Or if you only want to run one download format
```shell
cargo make download epub
```

### Suggesting Features

New features are always welcome but they need to have a issue associated first to discuss ways for a feature to be implemented, what the feature would do and how it would be implemented

### Issues related to image rendering

On terminals which implement image protocols such as [Wezterm](https://wezfurlong.org/wezterm/index.html) [iTerm2](https://iterm2.com/) there may be issues with how images are render, I have only used Wezterm on linux <br /> 

`manga-tui` will not render images on any other terminal that does not have image protocol, keep this in mind before making a issue about the image support  

## Pull Requests

Before making a pull request, please make an issue and then either fork this repo or make a branch that is intended to solve the issue

