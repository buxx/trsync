# trsync

![trsync illustration](illustration2.png)

Synchronize local folder with remote [Tracim](https://www.algoo.fr/fr/tracim) shared space.

## State of trsync

Trsync is in development. You can try it by following next sections.

## What is in this repository

### trsync

This is the tool permitting to synchronize one Tracim shared space with one local folder.

### manager

Daemon which manage multiples trsync executions by reading a config file.

### systray

Task bar icon program permitting to start a graphical configuration window to fill manager config file.

## What is not in this repository

### configure

The configuration windows program is available to [buxx/trsync-manager-configure](https://github.com/buxx/trsync-manager-configure)

## Build from source

Please install following dependencies, on linux :

    apt-get install build-essential pkg-config libssl-dev libsqlite3-dev

On Windows, install C++ build tools and sqlite3 dev.

For systray, install :

    apt-get install libpango1.0-dev libgtk-3-dev

Rust minimal required version is 1.56.0.

### trsync

Required : [Rust](https://www.rust-lang.org/tools/install)

1. Clone this repository
2. `cargo build --release --bin trsync` (`cargo build --features windows --release --bin trsync` if compiling with Windows)
3. Binary file available in `target/release`folder

### manager

1. Clone this repository
2. `cargo build --release --bin trsync_manager` (`cargo build --features windows --release --bin trsync_manager` if compiling with Windows)
3. Binary file available in `target/release`folder

### systray

1. Clone this repository
2. `cargo build --release --bin trsync_manager_systray` (`cargo build --features windows --release --bin trsync_manager_systray` if compiling with Windows)
3. Binary file available in `target/release`folder

## Usage

### trsync

Usage :

    trsync <path of folder to sync> <tracim address> <workspace id> <tracim username>

Example :

    cargo run ~/Tracim/MyProject mon.tracim.fr 42 bux

### manager

Create file at `~/trsync.conf` from `trsync.conf.tpl` and filled with your needs.

Then start `trsync_manager` binary.

### systray

The `libappindicator`package is required. Example for debian-like:

    apt-get install libappindicator3-1

You need [trsync-manager-configure](https://github.com/buxx/trsync-manager-configure) bin on your system.

You need a configuration file like at previous "manager" section.

Then start `trsync_manager_systray` binary.

#### ⚠ Debian 11

To be able to se the systray icon with Debian 11, you must install following gnome extension : https://extensions.gnome.org/extension/615/appindicator-support/
