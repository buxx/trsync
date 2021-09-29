# trsync

![trsync illustration](illustration2.png)

Synchronize local folder with remote [Tracim](https://www.algoo.fr/fr/tracim) shared space.

## State of trsync

trsync is in development. You can try it by following next sections.

## Run

You must have [rust](https://www.rust-lang.org/) programming language installed on you system.
From root of this repository, run :

    cargo run <path of folder to sync> <tracim address> <workspace id> <tracim username>

Example :

    cargo run ~/Tracim/MyProject mon.tracim.fr 42 bux
