# Ikki

Ikki is a tool for defining and running multi-container Docker applications. It is similar to Docker Compose but comes with some differences.

## Goals

The primary goal of Ikki is to make it possible to specify dependencies between multiple image builds. Consider the following two Dockerfiles:

```dockerfile
// Dockerfile.assets
FROM node:latest

WORKDIR /assets
// output assets to current workdir
```

```dockerfile
// Dockerfile.main
FROM node:latest

// Copy assets from previously built image
COPY --from=assets /assets ./assets
// start application
```

When building `Dockerfile.main`, Docker (by specification) will try to find image called `assets` locally. If it does not exist, it will try to pull it from the registry. It will *not* try to build it first, because there is no way to tell it to do so. A common solution is *multi-stage* builds but if more than one `Dockerfile` depends on the same base stage/image then duplication is needed. Docker Compose configuration does not help because it only allows to specify dependencies between running containers and not builds. This means that you have to give up declarative configuration partially to run some image builds in order manually. Ikki aims to preserve Compose-like configuration but also add declarative build dependency configuration in the same file. Ikki uses [KDL](https://kdl.dev/).

The secondary goal is to help the user avoid "vendor-locking" to Ikki and just migrate back to plain Docker CLI commands. This is done with `explain` command that just reads the Ikki config and translates the declarative configuration to imperative sequence of Docker commands that can be copy-pasted to any `bash` script as-is.

## Usage
```
Ikki container orchestrator for Docker

USAGE:
    ikki [OPTIONS] <SUBCOMMAND>

OPTIONS:
    -f, --file <FILE>    Path to Ikki configuration file [default: ikki.kdl]
    -h, --help           Print help information
    -V, --version        Print version information

SUBCOMMANDS:
    build
    explain
    help       Print this message or the help of the given subcommand(s)
    up         Build (or pull) all images and start the services
```

## Install
```
cargo install ikki
```
