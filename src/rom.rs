use std::error::Error;
use std::fmt::Write;

pub fn u8_to_bools(byte: &u8) -> Vec<bool> {
    let mut vec: Vec<bool> = Vec::new();

    for i in 0..8 {
        let masked_byte = (byte << i) & 128;
        if masked_byte == 128 {
            vec.push(true);
        } else {
            vec.push(false);
        }
    }

    vec.reverse();
    vec
}

pub fn bools_bin_str(bools: &[bool]) -> String {
    let mut ones_zeros: Vec<String> = bools
        .iter()
        .map(|b| {
            if *b {
                String::from("1")
            } else {
                String::from("0")
            }
        })
        .collect();
    ones_zeros.reverse();
    ones_zeros.join("")
}

pub fn create_rom(bools: &Vec<Vec<bool>>) -> Result<Vec<String>, Box<dyn Error>> {
    let rom_num = 0;

    let mut padded_bools = bools.clone();
    for _ in 0..(bools.len() % 8) {
        padded_bools.push(Vec::from([false; 16]));
    }

    let mut roms: Vec<String> = Vec::new();
    for rom_chip in padded_bools.chunks(8) {
        let mut rom = String::from("");
        writeln!(&mut rom, "CHIP ROM{} {{", rom_num)?;
        writeln!(&mut rom, "\tIN addr1[3], addr2[3];")?;
        writeln!(&mut rom, "\tOUT out1[16], out2[16];\n")?;
        writeln!(&mut rom, "\tPARTS:\n")?;

        for (inst_idx, inst) in rom_chip.iter().enumerate() {
            write!(&mut rom, "BufferGen<16>(\n\t")?;
            let port_mappings: Vec<String> = inst
                .iter()
                .enumerate()
                .map(|(bit_idx, bit)| format!("in[{bit_idx}]={bit},\n\t"))
                .collect();

            write!(&mut rom, "{}", port_mappings.join(""))?;
            writeln!(&mut rom, "out=rom{}", inst_idx)?;
            writeln!(&mut rom, "\t);\n")?;
        }

        writeln!(&mut rom, "\tMux8Way<16>(")?;
        writeln!(
            &mut rom,
            "\t\tin000=rom0, in001=rom1, in010=rom2, in011=rom3,"
        )?;
        writeln!(
            &mut rom,
            "\t\tin100=rom4, in101=rom5, in110=rom6, in111=rom7,"
        )?;
        writeln!(&mut rom, "\t\tsel=addr1[0..2],")?;
        writeln!(&mut rom, "\t\tout=out1")?;
        writeln!(&mut rom, "\t);")?;

        writeln!(&mut rom, "\tMux8Way<16>(")?;
        writeln!(
            &mut rom,
            "\t\tin000=rom0, in001=rom1, in010=rom2, in011=rom3,"
        )?;
        writeln!(
            &mut rom,
            "\t\tin100=rom4, in101=rom5, in110=rom6, in111=rom7,"
        )?;
        writeln!(&mut rom, "\t\tsel=addr2[0..2],")?;
        writeln!(&mut rom, "\t\tout=out2")?;
        writeln!(&mut rom, "\t);")?;

        writeln!(&mut rom, "}}")?;

        roms.push(rom);
    }

    Ok(roms)
}

#[cfg(test)]
mod romtest {
    use super::*;

    #[test]
    fn test_u8_to_bools_0() {
        let input: u8 = 0;
        let output = u8_to_bools(&input);
        let expected = Vec::from([false, false, false, false, false, false, false, false]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_1() {
        let input: u8 = 1;
        let output = u8_to_bools(&input);
        let expected = Vec::from([true, false, false, false, false, false, false, false]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_2() {
        let input: u8 = 2;
        let output = u8_to_bools(&input);
        let expected = Vec::from([false, true, false, false, false, false, false, false]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_3() {
        let input: u8 = 3;
        let output = u8_to_bools(&input);
        let expected = Vec::from([true, true, false, false, false, false, false, false]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_128() {
        let input: u8 = 128;
        let output = u8_to_bools(&input);
        let expected = Vec::from([false, false, false, false, false, false, false, true]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_129() {
        let input: u8 = 129;
        let output = u8_to_bools(&input);
        let expected = Vec::from([true, false, false, false, false, false, false, true]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_255() {
        let input: u8 = 255;
        let output = u8_to_bools(&input);
        let expected = Vec::from([true, true, true, true, true, true, true, true]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_bools_bin_str_0() {
        let input = Vec::from([false, false, false, false, false, false, false, false]);
        let output = bools_bin_str(&input);
        let expected = String::from("00000000");
        assert_eq!(output, expected);
    }

    #[test]
    fn test_bools_bin_str_3() {
        let input = Vec::from([true, true, false, false, false, false, false, false]);
        let output = bools_bin_str(&input);
        let expected = String::from("00000011");
        assert_eq!(output, expected);
    }

    #[test]
    fn test_bools_bin_str_255() {
        let input = Vec::from([true, true, true, true, true, true, true, true]);
        let output = bools_bin_str(&input);
        let expected = String::from("11111111");
        assert_eq!(output, expected);
    }
}
