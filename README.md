# Regex

A regular expression engine written in Rust. Implements Brzozwski derivatives in order to match a parsed regex pattern.

## Features

- Characters: a, b, c
- Wildcard: .
- Sequence: a + b
- Alternation: a | b
- Star: a*
- Plus: a+
- Optional: a?
- Range: a{m,n}
- Group: (...)
- Class: [abc]
- Anchors: ^, $
- Shorthands: \d, \w, \s

## How to build

### Prerequisites

Ensure that you have rust/cargo installed via [rustup](https://rust-lang.org/tools/install/)!

### Clone the github repository 

Open a terminal and execute the following:

```bash
git clone https://github.com/HDDyer/Regex.git
```

### Build and run the project

```bash
cd regex
cargo run
```

## License

Licensed under [MIT](./LICENSE)