# ORE CLI

A command line interface for the Ore program.

## **:black_nib:** Building

To build the Ore CLI, you will need to have the Rust programming language installed. You can install Rust by following the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

Once you have Rust installed, you can build the Ore CLI by running the following command:

```sh
cargo build --release
```

## Docker deploy

```
docker volume create portainer_data
```

```
docker run -d -p 8000:8000 -p 9000:9000 --name portainer --restart=always -v /var/run/docker.sock:/var/run/docker.sock -v portainer_data:/data portainer/portainer-ce:latest
```

Enter the `examples` folder and use nodejs to generate your yml file.

- create stack
- import yml
- deploy container

------

Reference repo: [ore-cli](https://github.com/HardhatChad/ore-cli)

thanks for the support by [sdevkc](https://github.com/sdevkc)
