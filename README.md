# Parterm

[![Rust](https://github.com/RazvanRotari/Parterm/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/RazvanRotari/Parterm/actions/workflows/rust.yml)

Remote control for your terminal. Allows you to execute arbitary commands in a different terminal. One usecase is to trigger the compilation in a different terminal from your editor.

This project only supports GNU/Linux at the moment. 

# Installation

### Cargo

If you already have a Rust environment set up, you can use the `cargo install` command:

    cargo install parterm

Cargo will build the `parterm` binary and place it in `$HOME/.cargo`.


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

