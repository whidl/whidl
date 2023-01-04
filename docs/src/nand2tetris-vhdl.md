# Nand2Tetris VDHL

This article will provide a demo usage of WHiDL to synthesize VHDL for at
least the DE1-SoC board. It would be nice if we added another piece of
hardware.

- Which nand2tetris solutions to use?

## Testing CPU

The Nand2Tetris tests for the full CPU can be run with the `whidl test`

## ROM synthesis

TODO: Script that synthesizes ROMs from Hack assembly files.

TODO: Instructions for plugging ROM into Computer.

## Testing computer

## Synthesizing VHDL

Does the CPU VHDL currently compile?
TODO: Make this command have consistent syntax

```
whidl synth-vhdl --output-dir cpu ./CPU.hdl
quartus_sh -t project.tcl
quartus_sh --flow compile CPU
```

Error

```
Error (10500): VHDL syntax error at PC.vhdl(40) near text "loop";  expecting an identifier File: /workspaces/whidl/resources/tests/nand2tetris/solutions/cpu/PC.vhdl Line: 40
Error (10500): VHDL syntax error at PC.vhdl(47) near text "loop";  expecting "(", or an identifier ("loop" is a reserved keyword), or  unary operator File: /workspaces/whidl/resources/tests/nand2tetris/solutions/cpu/PC.vhdl Line: 47
Error (10500): VHDL syntax error at PC.vhdl(51) near text "loop";  expecting "(", or an identifier ("loop" is a reserved keyword), or  unary operator File: /workspaces/whidl/resources/tests/nand2tetris/solutions/cpu/PC.vhdl Line: 51
Error (10500): VHDL syntax error at PC.vhdl(62) near text "loop";  expecting "end", or "(", or an identifier ("loop" is a reserved keyword), or a concurrent statement File: /workspaces/whidl/resources/tests/nand2tetris/solutions/cpu/PC.vhdl Line: 62
```
