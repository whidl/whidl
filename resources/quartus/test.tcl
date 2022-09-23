project_new test -overwrite
# Assign family, device, and top-level file
set_global_assignment -name FAMILY "Cyclone V"
set_global_assignment -name DEVICE 5CSEMA5F31C6
set_global_assignment -name TOP_LEVEL_ENTITY not_n2v
set_global_assignment -name VHDL_FILE Not.vhdl
set_global_assignment -name VHDL_FILE nand.vhdl
# Assign pins
#set_location_assignment -to clk Pin_28
#set_location_assignment -to clkx2 Pin_29
#set_location_assignment -to d[0] Pin_139
#set_location_assignment -to d[1] Pin_140
#
project_close

