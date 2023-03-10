# Trsync

![trsync illustration](illustration2.png)

Synchronize local folder with remote [Tracim](https://www.algoo.fr/fr/tracim) shared spaces.

## What is in this repository

* **trsync** : This is the tool permitting to synchronize one Tracim shared space with one local folder.
* **manager** : Daemon which manage multiples trsync executions by reading a config file.
* **configure** : The configuration window for manager config file
* **systray** : Task bar icon program permitting to start a graphical configuration window to fill manager config file.

## Build from source

GNU/Linux 🐧 : Please install following dependencies, example for Debian-like :

    apt-get install build-essential pkg-config libssl-dev libsqlite3-dev libpango1.0-dev libgtk-3-dev

(Note `libpango1.0-dev libgtk-3-dev` are only required for `systray` binary)  

`cargo` is also required (`apt-get install cargo` or install [latest](https://www.rust-lang.org/tools/install))

Windows : install C++ build tools, example with winget :

    winget install -e --id Microsoft.VisualStudio.2022.BuildTools

**Rust minimal required version is 1.56.0**.

### trsync

1. From root of this repository
2. **GNU/Linux 🐧** : `cargo build --release --bin trsync`. **Windows** : `cargo build --features windows --release --bin trsync`
3. Binary file available in `target/release`folder

### manager

1. Clone this repository
2. **GNU/Linux 🐧** : `cargo build --release --bin trsync_manager`. **Windows** : `cargo build --features windows --release --bin 
3. Binary file available in `target/release`folder

### systray

1. Clone this repository
2. **GNU/Linux 🐧** : `cargo build --release --bin trsync_manager_systray`. **Windows** : `cargo build --features windows --release --bin trsync_manager_systray`
3. Binary file available in `target/release`folder

## Usage

### trsync

Usage :

    trsync <path of folder to sync> <tracim address> <workspace id> <tracim username>

Example :

    cargo run ~/Tracim/MyProject mon.tracim.fr 42 bux

User password will be asked by prompt. To use environment variable, indicate environment variable containg password name with `--env-var-pass PASSWORD` where `PASSWORD` is the environment variable name.

### manager

Create file at `~/trsync.conf` (by copying `trsync.conf.tpl`) and fill it with your needs.

`trsync_manager` will try to get passwords from system secret manager with key `trsync::<instance name>` and current logged user as user. To set raw password, add `password` key with password as value in each instances sections.

Then start `trsync_manager` binary.

### systray

The `libappindicator` package is required. Example for debian-like:

    apt-get install libappindicator3-1

⚠ If you run Debian 11 + Gnome Shell, you must install following gnome extension : https://extensions.gnome.org/extension/615/appindicator-support/.

You need [trsync-manager-configure](https://github.com/buxx/trsync-manager-configure) on your system.

You need a configuration file like at previous "manager" section.

Then start `trsync_manager_systray` binary.

## Deployment on your OS

See [deployment doc file](doc/deployment.md)
