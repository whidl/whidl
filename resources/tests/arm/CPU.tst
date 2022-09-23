load CPU.hdl,
output-file CPU.out,
compare-to CPU.cmp,
output-list instruction%B3.16.3 outM%B3.16.3 writeM%B3.1.3 addressM%B3.16.3;

// movs r0, #2
// movs r1, #0
// str r0, [r1]
set instruction %B0010000000000010,
tick, tock, output;

set instruction %B0010000100100001,
tick, tock, output;

set instruction %B0110000001100000,
tick, tock, output;
