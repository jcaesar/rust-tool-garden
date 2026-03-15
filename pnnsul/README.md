# PNNSUL

*Port network namespace unix listener*

A little *peninsula*.

## Purpose

You want something to listen on a unix socket, but it only supports listening on a TCP port.
Maybe you have a port collision, or maybe you just don't want every other process on your machine and every website you open in your browser to be able to connect to it.
This little tool will listen create a new network namespace, spawn a process on it 

## Usage example
```bash
pnnsul --listen ./sock --connect 8000 -- python -m http.server
xh --unix-socket ./sock :/ \
|| curl --unix-socket ./sock http://-./Cargo.toml
```

## Installation
```bash
cargo install --git https://github.com/jcaesar/pnnsul
# or:
nix run github:jcaesar/pnnsul
```

## Comparison
This is essentially just
```shell
$ unshare --net --user --map-root-user
# ip link set dev lo up
# socat UNIX-LISTEN:/tmp/sock,fork TCP-CONNECT:127.0.0.1:$PORT &
# $EXEC
```
But it has a nice few extra gimmicks like not accepting connections until the TCP listener is actually running.
