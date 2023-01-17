# Loops

For loops are easy to use, and they work similarly to how they do in other languages. The general format of a for loop is:
`FOR <variable> IN <range> GENERATE { body of loop between curly braces.. }`


The following example uses a loop to NOT every bit of a byte input.
``` hdl
CHIP NotGen {
    IN in[8];
    OUT out[8];

    PARTS:
    
    FOR i IN 0 TO 7 GENERATE {
        Not(in=in[i], out=out[i]);
    }
}
```

You can use generic values within the for-loop too if you want.
``` hdl
CHIP NotGen<X> {
    IN in[X];
    OUT out[X];

    PARTS:
    FOR i IN 0 TO X-1 GENERATE {
        Not(in=in[i], out=out[i]);
    }
}
```
