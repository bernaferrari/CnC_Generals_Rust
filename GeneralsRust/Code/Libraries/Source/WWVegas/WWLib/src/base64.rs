const PAD: u8 = b'=';
const ENCODER: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

const BAD: u8 = 0xFE;
const END: u8 = 0xFF;

static DECODER: [u8; 256] = {
    let mut table = [BAD; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = BAD;
        i += 1;
    }
    table[b'+' as usize] = 62;
    table[b'/' as usize] = 63;
    table[b'0' as usize] = 52;
    table[b'1' as usize] = 53;
    table[b'2' as usize] = 54;
    table[b'3' as usize] = 55;
    table[b'4' as usize] = 56;
    table[b'5' as usize] = 57;
    table[b'6' as usize] = 58;
    table[b'7' as usize] = 59;
    table[b'8' as usize] = 60;
    table[b'9' as usize] = 61;
    table[b'=' as usize] = END;

    table[b'A' as usize] = 0;
    table[b'B' as usize] = 1;
    table[b'C' as usize] = 2;
    table[b'D' as usize] = 3;
    table[b'E' as usize] = 4;
    table[b'F' as usize] = 5;
    table[b'G' as usize] = 6;
    table[b'H' as usize] = 7;
    table[b'I' as usize] = 8;
    table[b'J' as usize] = 9;
    table[b'K' as usize] = 10;
    table[b'L' as usize] = 11;
    table[b'M' as usize] = 12;
    table[b'N' as usize] = 13;
    table[b'O' as usize] = 14;
    table[b'P' as usize] = 15;
    table[b'Q' as usize] = 16;
    table[b'R' as usize] = 17;
    table[b'S' as usize] = 18;
    table[b'T' as usize] = 19;
    table[b'U' as usize] = 20;
    table[b'V' as usize] = 21;
    table[b'W' as usize] = 22;
    table[b'X' as usize] = 23;
    table[b'Y' as usize] = 24;
    table[b'Z' as usize] = 25;

    table[b'a' as usize] = 26;
    table[b'b' as usize] = 27;
    table[b'c' as usize] = 28;
    table[b'd' as usize] = 29;
    table[b'e' as usize] = 30;
    table[b'f' as usize] = 31;
    table[b'g' as usize] = 32;
    table[b'h' as usize] = 33;
    table[b'i' as usize] = 34;
    table[b'j' as usize] = 35;
    table[b'k' as usize] = 36;
    table[b'l' as usize] = 37;
    table[b'm' as usize] = 38;
    table[b'n' as usize] = 39;
    table[b'o' as usize] = 40;
    table[b'p' as usize] = 41;
    table[b'q' as usize] = 42;
    table[b'r' as usize] = 43;
    table[b's' as usize] = 44;
    table[b't' as usize] = 45;
    table[b'u' as usize] = 46;
    table[b'v' as usize] = 47;
    table[b'w' as usize] = 48;
    table[b'x' as usize] = 49;
    table[b'y' as usize] = 50;
    table[b'z' as usize] = 51;

    table
};

pub fn base64_encode(source: &[u8], dest: &mut [u8]) -> usize {
    if source.is_empty() || dest.is_empty() {
        return 0;
    }

    let mut slen = source.len();
    let mut dlen = dest.len();
    let mut sptr = 0usize;
    let mut dptr = 0usize;
    let mut total = 0usize;

    while slen > 0 && dlen >= 4 {
        let mut c1 = source[sptr];
        sptr += 1;
        slen -= 1;
        let mut pad = 0usize;

        let mut c2 = 0u8;
        if slen > 0 {
            c2 = source[sptr];
            sptr += 1;
            slen -= 1;
        } else {
            pad += 1;
        }

        let mut c3 = 0u8;
        if slen > 0 {
            c3 = source[sptr];
            sptr += 1;
            slen -= 1;
        } else {
            pad += 1;
        }

        let o1 = (c1 >> 2) & 0x3F;
        let o2 = ((c1 & 0x03) << 4) | ((c2 >> 4) & 0x0F);
        let o3 = ((c2 & 0x0F) << 2) | ((c3 >> 6) & 0x03);
        let o4 = c3 & 0x3F;

        dest[dptr] = ENCODER[o1 as usize];
        dest[dptr + 1] = ENCODER[o2 as usize];
        dest[dptr + 2] = if pad < 2 { ENCODER[o3 as usize] } else { PAD };
        dest[dptr + 3] = if pad < 1 { ENCODER[o4 as usize] } else { PAD };

        dptr += 4;
        dlen -= 4;
        total += 4;
    }

    if dlen > 0 {
        dest[dptr] = 0;
    }

    total
}

pub fn base64_decode(source: &[u8], dest: &mut [u8]) -> usize {
    if source.is_empty() || dest.is_empty() {
        return 0;
    }

    let mut slen = source.len();
    let mut sptr = 0usize;
    let mut dptr = 0usize;
    let mut dlen = dest.len();
    let mut total = 0usize;

    while slen > 0 && dlen > 0 {
        let mut packet = [0u8; 4];
        let mut pcount = 0usize;

        while pcount < 4 && slen > 0 {
            let ch = source[sptr];
            sptr += 1;
            slen -= 1;

            let code = DECODER[ch as usize];
            if code == BAD {
                continue;
            }
            if code == END {
                slen = 0;
                break;
            }

            packet[pcount] = code;
            pcount += 1;
        }

        let b1 = (packet[0] << 2) | (packet[1] >> 4);
        dest[dptr] = b1;
        dptr += 1;
        dlen -= 1;
        total += 1;

        if dlen > 0 && pcount > 2 {
            let b2 = (packet[1] << 4) | (packet[2] >> 2);
            dest[dptr] = b2;
            dptr += 1;
            dlen -= 1;
            total += 1;
        }

        if dlen > 0 && pcount > 3 {
            let b3 = (packet[2] << 6) | packet[3];
            dest[dptr] = b3;
            dptr += 1;
            dlen -= 1;
            total += 1;
        }
    }

    total
}
