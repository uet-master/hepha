# Hepha 🦔

Hepha tool analyzes smart contracts written by Rust language to discover vulnerabilities. Hepha only detects six types of vulnerabilities as belows.

- Reentrancy
- Underflow
- Overflow
- Bad randomness
- Time manipulation
- Numerical precision

## Installation Instructions

Install dependencies

- Rust using rustup. You can find the installation instructions [here](https://doc.rust-lang.org/book/ch01-01-installation.html).
- Cmake. The installation instructions can be found [here](https://cmake.org/install/).
- Clang. The installation instructions can be followed [here](https://clang.llvm.org/get_started.html).

Install hepha into cargo

```bash
cargo install --locked --path ./checker
```

To evaluate a smart contract, run the command below over its directory

```bash
cargo hepha
```
