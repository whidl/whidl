# WHiDL

![cargo build](https://github.com/whidl/whidl/actions/workflows/build.yml/badge.svg)
![cargo test](https://github.com/whidl/whidl/actions/workflows/test.yml/badge.svg)

WHiDL is a source-to-source compiler that translates Nand2Tetris HDL to VHDL,
making it possible to run HDL programs on an FPGA while retaining (most) of the
simplicity of the original HDL.

## Quick Start

The contents of the `.devcontainer` contains the development container
configuration. I try to keep `devcontainer.json` up to date and able to
produce a working development environment. There are a few couple of options
for building the development environment, depending on your opinion of
VS Code and cloud services ⛈️⛈️⛈️.

There is not currently separate deployment and development environments. To
run WHiDL the easiest way is to use the development environment and build
it with cargo.

**Please be aware that the development docker image over 20GB due to the dependencies
on Quartus Prime and Modelsim. If you are using GitHub codespaces, select
a configuration with at least 64GB of disk space. This also means that 
building the docker image can take quite some time. Grab a coffee.**

You have five development environment options, sorted from least dependent
on Microsoft to most dependent on Microsoft:

0. You can read the files in the `.devcontainer` directory and figure out how to build the
   development environment yourself. Look at the `Dockerfile` and `devcontainer.json`. This
   is straightforward if you are familiar with docker and devcontainers.
1. You can use the devcontainer CLI to build the docker image and run WHiDL from
   the command line. See the `Terminal + Local Docker` section below.
2. You can use the GitHub CLI to create a codespace and SSH into it without using VS
   Code. See the `Terminal + Codespaces` section below.
3. You can use VS Code and the VS Code Remote - Containers extension to build
   the docker image and run WHiDL locally. See the `VS Code + Local container` section
   below.
4. You can use VS Code and the VS Code Remote - Codespaces extension to build
   the docker image and run WHiDL in the cloud. See the `VS Code with Codespaces`
   section below.

### Terminal + Local Docker

If you want to run WHiDL on your local machine without VS Code, you can
use the devcontainers cli to build the container.

First install the devcontainers cli with `npm install -g @devcontainer/cli`

Then on an x86 machine run the devcontainer command below. WHiDL is not
currently supported on ARM machines due to the Quartus Prime dependencies. 

```
devcontainer exec --workspace-folder whidl cargo test --release
```

See the `Using WHiDL` section below for more information on running WHiDL.

WHiDL is not currently supported on ARM machines due to the Quartus Prime
dependencies. 

#### Terminal + Codespaces

If you want to run WHiDL on GitHub codespaces without VS Code you can
use the GitHub CLI to create a codespace and the SSH into it.

Run `gh codespace create` to create a codespace. Use `whidl/whidl` as the
repository and the `main` branch.  Once the codespace is created, you can SSH
into it with `gh codespace ssh`.

See the `Using WHiDL` section below for more information on running WHiDL.

#### VS Code + Local container

If you want to use VS Code, but run the container locally without Codespaces,
you can use the VS Code Remote - Containers extension to open the repository in
a container. If you have docker installed this should "just work."

See the `Using WHiDL` section below for more information on running WHiDL.

#### VS Code with Codespaces

If you want to use VS Code and run the container in the cloud, you can use the
VS Code Remote - Codespaces extension to open the repository in a codespace.

See the `Using WHiDL` section below for more information on running WHiDL.

### Building and using WHiDL

After you have built the development environment, run the test suite with
`cargo test --release` to verify that everything is working.

You can build the WHiDL binary with `cargo build --release`. The binary output
wkll be `target/release/whidl`.

To see WHiDL in action, convert the Nand2Tetris Mux test into VHDL with the following command:

```shell
target/release/whidl synth-vhdl resources/tests/nand2tetris/solutions/Mux.tst MuxQuickstart
```

This command will convert the Nand2Tetris Mux test and all component chips into
VHDL, generate a Quartus Prime project for the DE1-SoC board, generate
a Modelsim testbench, and place these files in the newly created `MuxQuickstart` directory.

The two primary output files are:
* `Mux.tst.vhdl` - the testbench to run under Modelsim
* `Mux.vhdl` - the VHDL for the Mux chip itself

### Running the Modelsim tests

You can run the Modelsim tests with the following commands from the output directory:

```shell
vlib work
vcom *.vhdl
vsim -c Mux_tst -do "run 100ns; quit"
```

### Compiling the Quartus Prime project

```shell
quartus_sh -t project.tcl
quartus_sh --flow compile Mux
```

The output file, `Mux.sof`, can be used by the Quartus Prime programmer to program a DE1-SoC board.
The details of using the Quartus Prime programmer are beyond the scope of this guide.

## Docs

In-progress documentation is at [whidl.io](https://whidl.io/). 
The docs source is the `docs` directory in this repository. See `docs/README.md`
for more information.

## Development

### Version Changing

(Reminder to devs)
- Make sure to change the version in `Cargo.toml` and `package.json.publish`.
