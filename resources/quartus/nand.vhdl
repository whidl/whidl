library ieee;
use ieee.std_logic_1164.all;

entity nand_n2v is
port (a : in std_logic_vector(0 downto 0);
b : in std_logic_vector(0 downto 0);
out_n2v : out std_logic_vector(0 downto 0)
);
end entity nand_n2v;

architecture arch of nand_n2v is
begin
out_n2v <= a nand b;
end architecture arch;


