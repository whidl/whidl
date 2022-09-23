// This file is part of www.nand2tetris.org
// and the book "The Elements of Computing Systems"
// by Nisan and Schocken, MIT Press.
// File name: projects/01/Mux8Way16.tst

load<3> Mux8Way.hdl,
output-file Mux8Way3.out,
compare-to Mux8Way3.cmp,
output-list in000%B2.3.2 in001%B2.3.2 in010%B2.3.2 in011%B2.3.2 in100%B2.3.2 in101%B2.3.2 in110%B2.3.2 in111%B2.3.2 sel%B2.3.2 out%B1.3.1;

set in000 0,
set in001 0,
set in010 0,
set in011 0,
set in100 0,
set in101 0,
set in110 0,
set in111 0,
set sel 0,
eval,
output;

set sel 1,
eval,
output;

set sel 2,
eval,
output;

set sel 3,
eval,
output;

set sel 4,
eval,
output;

set sel 5,
eval,
output;

set sel 6,
eval,
output;

set sel 7,
eval,
output;

set in000 0,
set in001 1,
set in010 2,
set in011 3,
set in100 4,
set in101 5,
set in110 6,
set in111 7,
set sel 0,
eval,
output;

set sel 1,
eval,
output;

set sel 2,
eval,
output;

set sel 3,
eval,
output;

set sel 4,
eval,
output;

set sel 5,
eval,
output;

set sel 6,
eval,
output;

set sel 7,
eval,
output;
