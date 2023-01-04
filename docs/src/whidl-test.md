# Running Tests

TODO: Fix requirement that basepaths are used.
TODO: Support Nand2Tetris full computer tess?

Nand2Tetris tests can be run with the command `whidl test --test-file ./TESTFILE.tst`
WHiDL supports the Nand2Tetris tests that don't use loops. This is all of the
chip tests up through the CPU, but not the full computer.

For example, if `And.tst` is a test file in the current directory, then run
`whidl test --test-file ./And.tst`. WHiDL expected the HDL files to be in
the same directory as the test file.

## Using generics in test scripts

Tests scripts for chips that declare one or more generic variables must be
tested using a specific value of the generic variables. These specific values
are chosen using brackets around the `load` command in test scripts.

For example, here is a version of the `Register` chip that stores `X` bits,
where `X` is a generic variable.

```
CHIP Register<W> {
    IN in[W], load;
    OUT out[W];

    PARTS:
    FOR i IN 0 TO W-1 GENERATE {
        Bit(in=in[i], load=load, out=out[i]);
    }
}
```

The corresponding test script must choose a value for `X` to test. Here we
choose the value 8 to make the tests compatible with the existing Register
comparison file.

```
load<8> Register.hdl,
output-file Register.out,
compare-to Register.cmp,
output-list time%S1.4.1 in%D1.6.1 load%B2.1.2 out%D1.6.1;

set in 0,
set load 0,
tick,
output;

// Tests continue...
```
