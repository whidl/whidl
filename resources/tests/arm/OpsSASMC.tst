load OpsSASMC.hdl,
output-file OpsSASMC.out,
compare-to OpsSASMC.cmp,
output-list instruction%B3.16.3 Rd%B3.3.3 Rm%B3.3.3;

// movs r7, #3
set instruction %B10011100000011,
eval, output;
