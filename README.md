# WHiDL

![cargo build](https://github.com/whidl/whidl/actions/workflows/build.yml/badge.svg)
![cargo test](https://github.com/whidl/whidl/actions/workflows/test.yml/badge.svg)

WHiDL is a source-to-source compiler that translates Nand2Tetris HDL to VHDL,
making it possible to run HDL programs on an FPGA while retaining (most) of the
simplicity of the original HDL.

## Quick Start

The following steps will guide you through the process of getting WHiDL running
on GitHub Codespaces and converting a simple Mux chip from HDL to VHDL.

### Prerequisites

Ensure that your codespace configuration has a minimum of 64GB of disk space.
Selecting one of the smaller two codespace configurations (2 or 4 cores) may
result in running out of disk space and subsequent build failure.

### Building WHiDL

1. Create a codespace from this repository.
2. Once the codespace has finished building, execute the following commands to build and test WHiDL:

```shell
cargo build --release
cargo test --release
```

### Using WHiDL

1. From the root of the source repository, execute the following command:

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

- Make sure to change the version in `Cargo.toml` and `package.json.publish`.
