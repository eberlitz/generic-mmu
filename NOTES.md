## Setup

Rust for ESP32 (no_std):
If you're using Rust for ESP32 in a no_std environment, you'll need to set up the esp-rs toolchain. Here's what you should do:

a. Install the Rust toolchain for ESP32:

```sh
cargo install cargo-espflash --locked
cargo install cargo-espmonitor
```

b. Install the Xtensa toolchain:
You can use the esp-rs/espup tool to install the required toolchain:

```sh
cargo install espup
espup install
```

### Creating a project using a template:

https://docs.esp-rs.org/book/writing-your-own-application/generate-project/index.html

```sh
cargo install cargo-generate
cargo generate esp-rs/esp-template
```

### running

```sh
source ~/export-esp.sh
sh ./scripts/run.sh
```
