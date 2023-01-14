# WHiDL

![cargo build](https://github.com/whidl/whidl/actions/workflows/build.yml/badge.svg)
![cargo test](https://github.com/whidl/whidl/actions/workflows/test.yml/badge.svg)

## Documentation

In-progress documentation is at [whidl.github.io/whidl](https://whidl.github.io/whidl). The docs source is
the `docs` directory in this repository. The documentation can be read
locally by using `mdbook serve`, or on GitHub codespaces `mdbook serve --hostname 0.0.0.0`. 

The documentation at whidl.github.io/whidl is updated by any push to the `main`
branch that changes files in `doc/**`.

## Using whidl to synthesize ROMs for the CS 314 Toy ARM computer

The `rom` subcommand can be used to synthesize ROM files for the Toy
ARM computer built in CS 314.

### Creating thumb binary

Depends on package `binutils-arm-none-eabi`

```
arm-none-eabi-as -march=armv7-a -mthumb file.s -o $file
```

### Creating ROM HDL

Then run `whidl rom $file`. This will print out the ROM HDL files to
standard out.

## Development

### Version Changing

- Make sure to change the version in `Cargo.toml` and `package.json.publish`.



Synthesizing and running Modelsim tests on VHDL code from nand2tetris tests.


