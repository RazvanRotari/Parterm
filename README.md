# Parterm

[![Rust](https://github.com/RazvanRotari/Parterm/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/RazvanRotari/Parterm/actions/workflows/rust.yml)

Remote control for your terminal. Allows you to execute arbitary commands in a different terminal. One usecase is to trigger the compilation in a different terminal from your editor.

This project only supports GNU/Linux at the moment. 

# Installation

You can use `cargo install` to install this project. It will compile the binary `parterm` and install it in the `~/.cargo/bin` folder. Make sure this folder is in your path if you want to be able to run it directly.


This project is not ready to be used yet. Once the project is ready, it will be published on crates.io and you will be able to install the latest release with a simple `cargo install parterm`.

# Usage

Start the server
```
parterm server
```

In another terminal run
```
parterm client -- ls
```

This will run "ls" in the first terminal.

