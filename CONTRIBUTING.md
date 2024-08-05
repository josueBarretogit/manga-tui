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


### Suggesting Features

New features are always welcome but they need to have a issue associated first to discuss ways for a feature to be implemented, what the feature would do and how it would be implemented

### Issues related to image rendering

On terminals which implement image protocols such as [Wezterm](https://wezfurlong.org/wezterm/index.html) [iTerm2](https://iterm2.com/) there may be issues with how images are render, I have only used Wezterm on linux <br /> 

`manga-tui` will not render images on any other terminal that does not have image protocol, keep this in mind before making a issue about the image support  

## Pull Requests

Before making a pull request, please make an issue and then either fork this repo or make a branch that is intended to solve the issue
