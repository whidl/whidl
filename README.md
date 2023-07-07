# WHiDL

![cargo build](https://github.com/whidl/whidl/actions/workflows/build.yml/badge.svg)
![cargo test](https://github.com/whidl/whidl/actions/workflows/test.yml/badge.svg)

## Quick Start

The fastest way to see WHiDL in action is to use GitHub codespaces. 

Create a Codespace from this repository using a configuration that has at least
64GB of disk space. If you select one of the smaller two codespace configurations
(2 or 4 cores), the codespace will run out of space and fail to build.

Once the codespace has finished building,  run
`cargo build --release` and then  `cargo test --release` to verify that WHiDL is ready to go.

From the root of the source repository, run 
`target/release/whidl synth-vhdl resources/tests/nand2tetris/solutions/Mux.tst MuxQuickstart'.  This will convert
the Nand2Tetris HDL for a Mux chip into VHDL and generate a quartus prime
project for the DE1-SoC board. The output file `Mux.tst.vhdl` is the testbench
to run under Modelsim, and Mux.vhdl is the VHDL for the Mux chip itself.

Compiling the VHDL code is a two step process:

1. Run `quartus_sh -t project.tcl`
2. Run `quartus_sh --flow compile Mux`

The output file is `Mux.sof` which can be used by the Quartus Prime programmer
to program a DE1-SoC board.

In lieu of programming a board, you can also run the Modelsim tests with
the following commands (from the output directory).

```
vlib work
vcom *.vhdl
vsim -c Mux_tst -do "run 100ns; quit"
```

## Documentation

In-progress documentation is at [whidl.io](https://whidl.io/). 
The docs source is the `docs` directory in this repository. See `docs/README.md`
for more information.

## Development

### Version Changing

- Make sure to change the version in `Cargo.toml` and `package.json.publish`.

