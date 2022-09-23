Assembly files with:

```
arm-none-eabi-as -march-armv7-a -mthumb file.s
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
