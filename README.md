# nand2vhdl

## Version Changing

- Make sure to change the version in `Cargo.toml` and `package.json.publish`.

## Using whidl to synthesize ROMs for the CS 314 Toy ARM computer

Requires these packages for assembling:

`binutils-arm-none-eabi`

Assembly files with:

```
arm-none-eabi-as -march=armv7-a -mthumb file.s -o file
```

Extract binary instructions with:

Use `readelf -S a.out` to get the start of the `.text` section.


Where N is the number of assembly instructions in the program - 1
Assumes first instruction at address 34 (verify with readelf)
where N is number of instructions, e.g. for 1 instruction

```
N=1
xxd -e -g 2 -c2 -s 52 -l $(echo "${N}*2" | bc) -u a.out | while read line; do
    hex=$(echo $line | awk '{print $2}')
    printf "%016d\n" $(echo -e "ibase=16; obase=2; $hex" | bc)
done
```
