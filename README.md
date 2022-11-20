# whidl

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
