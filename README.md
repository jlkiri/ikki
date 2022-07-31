# Ikki

![crates.io](https://img.shields.io/crates/v/ikki.svg)

Ikki is a tool for defining and running multi-container Docker applications. It is similar to Docker Compose but comes with some differences.

## Goals

* Possible to make one image build dependent on another
* Possible to "unmigrate" easily
* Watch filesystem and rebuild what is necessary

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

When building `Dockerfile.main`, Docker (by specification) will try to find image called `assets` locally. If it does not exist, it will try to pull it from the registry. It will *not* try to build it first, because there is no way to tell it to do so. A common solution is [*multi-stage*](https://docs.docker.com/develop/develop-images/multistage-build/) builds but if more than one `Dockerfile` depends on the same base stage/image then duplication is needed. Docker Compose configuration does not help because it only allows to specify dependencies between running containers and not builds. This means that you have to give up declarative configuration partially to run some image builds in order manually. Ikki aims to preserve Compose-like configuration but also add declarative build dependency configuration in the same file. Ikki uses [KDL](https://kdl.dev/).

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

## Configuration

Ikki uses [KDL](https://kdl.dev/) for configuration. By default it looks for configuration in `ikki.kdl` file. The (unfinished) schema can be found in `ikki-config\schema.kdl`. Currently the schema is not enforced and `knuffel` library is used instead.

The `images` node is basically what you would normally find in a Docker Compose file. Only those images that have a `service` configuration are run as containers with the `up` command.

The `dependencies` node is a DAG that specifies the dependencies. Names should match images names under the `images` configuration.

### Example

```kdl
images {
    image "protobuf" path="./protobuf" output="./output/protobuf" {
        build-arg "PROTOBUF_VERSION" "1.28.0"
        build-arg "PROTOC_VERSION" "21.4"
    }

    image "redis" pull="redis:latest" {
        service {
            ports "6379:6379"
        }
    }
    
    image "db" pull="postgres:latest" {
        service {
            env "POSTGRES_PASSWORD" "example"
            env "POSTGRES_USER" "test"

            ports "5432:5432"
        }
    }

    image "api" path="./api" {
        service {
            mount type="volume" src="cache" dest="/cache"
            mount type="bind" src="./api/config" dest="/config"

            ports "3000:3000"
        }
    }

    image "cli-rust" path="./cli"
}

dependencies {
    api {
        protobuf
        redis
        db
    }
    cli-rust {
        protobuf
    }
}
```

### Explain

By using the `explain` command it is possible to turn the above config into a sequence of Docker commands:

```
❯ ikki explain
docker build --build-arg PROTOBUF_VERSION=1.28.0 --build-arg PROTOC_VERSION=21.4 --tag protobuf ./protobuf
docker pull redis:latest
docker pull postgres:latest
docker build --tag api ./api
docker build --tag cli-rust ./cli
docker run --name redis --publish 6379:6379 redis:latest
docker run --name db --env POSTGRES_PASSWORD=example --env POSTGRES_USER=test --publish 5432:5432 postgres:latest
docker run --name api --publish 3000:3000 api
```

## Status

**Experimental**

Use at your own risk. The following is the rough TODO list:

- [ ] Reach parity with Docker Compose by recognizing more options and passing them to the Docker daemon
- [ ] Distinguish build and run dependencies

## Install

### From source
```
cargo install ikki
```
