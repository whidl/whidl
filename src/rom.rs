fn u8_to_bool(byte: &u8) -> Vec<bool> {
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

#[cfg(test)]
mod romtest {
    use super::*;

    #[test]
    fn test_u8_to_bools_zero() {
        let input: u8 = 0;
        let output = u8_to_bool(&input);
        let expected = Vec::from([false, false, false, false, false, false, false, false]);
        assert_eq!(output, expected);
    }
    
    #[test]
    fn test_u8_to_bools_one() {
        let input: u8 = 1;
        let output = u8_to_bool(&input);
        let expected = Vec::from([false, false, false, false, false, false, false, true]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_two() {
        let input: u8 = 2;
        let output = u8_to_bool(&input);
        let expected = Vec::from([false, false, false, false, false, false, true, false]);
        assert_eq!(output, expected);
    }
    
    #[test]
    fn test_u8_to_bools_three() {
        let input: u8 = 3;
        let output = u8_to_bool(&input);
        let expected = Vec::from([false, false, false, false, false, false, true, true]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_128() {
        let input: u8 = 128;
        let output = u8_to_bool(&input);
        let expected = Vec::from([true, false, false, false, false, false, false, false]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_129() {
        let input: u8 = 129;
        let output = u8_to_bool(&input);
        let expected = Vec::from([true, false, false, false, false, false, false, true]);
        assert_eq!(output, expected);
    }

    #[test]
    fn test_u8_to_bools_255() {
        let input: u8 = 255;
        let output = u8_to_bool(&input);
        let expected = Vec::from([true, true, true, true, true, true, true, true]);
        assert_eq!(output, expected);
    }
}
