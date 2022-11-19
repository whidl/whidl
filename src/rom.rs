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

    vec
}

pub fn bools_bin_str(bools: &Vec<bool>) -> String {
    let mut s = String::from("");
    for b in bools {
        if *b {
            s.push('1');
        } else {
            s.push('0');
        }
    }

    s
}

pub fn create_rom(bools: &Vec<Vec<bool>>) -> Vec<String> {
    let rom_num = 0;
    for rom_chip in bools.chunks(8) {
        println!("CHIP ROM{}", rom_num);
    }

    Vec::new()
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
        let expected = Vec::from([false, false, false, false, false, false, false, true]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_2() {
        let input: u8 = 2;
        let output = u8_to_bools(&input);
        let expected = Vec::from([false, false, false, false, false, false, true, false]);
        assert_eq!(output, expected);
    }
    
    #[test]
    fn test_u8_to_bools_3() {
        let input: u8 = 3;
        let output = u8_to_bools(&input);
        let expected = Vec::from([false, false, false, false, false, false, true, true]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_128() {
        let input: u8 = 128;
        let output = u8_to_bools(&input);
        let expected = Vec::from([true, false, false, false, false, false, false, false]);
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
        let input = Vec::from([false, false, false, false, false, false, true, true]);
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
