# Using whidl
First off, if you ever can't remember a command in whidl, type `whidl -h` or `whidl --help` to list them. You can then type `whidl <name of command> -h` to show the available options for that command (for example, `whidl check -h` shows the options for the check command).


### Check your code
Use the `check` command to detect any errors in your code. Note that this **does not** test your code, it just checks to see if your code if "well-formed". In other words, it doesn't tell you if your code runs correctly, it just tells you if it runs. You tell whidl what file to check via the `--top-level-file` flag.

Example:
`whidl check --top-level-file my-chip/MyChip.hdl`


### Run tests
To test hdl you can run `whidl test --test-file <name of test file>` where the test file is a test script.

Example: 
`whidl test --test-file my-hdl/MyChip.tst`


### Generate vhdl
The `synth-vhdl` command generates vhdl from and hdl file that can be run through quartus on an FPGA. You have to tell whidl where you want it to put the vhdl that it generates using the `--output-dir` option.

Example:
`whidl synth-vhdl --output-dir my-vhdl my-hdl/SomeChip.hdl`

Note that you don't supply the entire directory of your project. Just a single hdl file.


### Additional commands
Any other commands used by whidl are context-specific, and are not used for working with vhdl or hdl.
