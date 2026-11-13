const SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const CRC_TABLE: [u32; 256] = crc_table();

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

pub fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |c, &byte| {
        CRC_TABLE[((c ^ u32::from(byte)) & 0xFF) as usize] ^ (c >> 8)
    })
}

pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for block in data.chunks(5552) {
        for &byte in block {
            a += u32::from(byte);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row = width as usize * 4;
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgba.chunks_exact(row.max(1)).take(height as usize) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(usize::from(u16::MAX)).collect();
    if blocks.is_empty() {
        zlib.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    for (i, block) in blocks.iter().enumerate() {
        let len = block.len() as u16;
        zlib.push(u8::from(i + 1 == blocks.len()));
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut png = SIGNATURE.to_vec();
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib);
    chunk(&mut png, b"IEND", &[]);
    png
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

    fn chunks(png: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut at = SIGNATURE.len();
        while at < png.len() {
            let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let kind = &png[at + 4..at + 8];
            let data = &png[at + 8..at + 8 + len];
            let stored = u32::from_be_bytes(png[at + 8 + len..at + 12 + len].try_into().unwrap());
            let mut covered = kind.to_vec();
            covered.extend_from_slice(data);
            assert_eq!(crc32(&covered), stored, "CRC of chunk");
            out.push((String::from_utf8(kind.to_vec()).unwrap(), data.to_vec()));
            at += 12 + len;
        }
        out
    }

    fn inflate_stored(zlib: &[u8]) -> Vec<u8> {
        assert_eq!(zlib[0], 0x78);
        assert_eq!((u16::from(zlib[0]) << 8 | u16::from(zlib[1])) % 31, 0);
        let mut out = Vec::new();
        let mut at = 2;
        loop {
            let header = zlib[at];
            assert_eq!(header & 0b110, 0, "stored block");
            let len = u16::from_le_bytes([zlib[at + 1], zlib[at + 2]]) as usize;
            let nlen = u16::from_le_bytes([zlib[at + 3], zlib[at + 4]]);
            assert_eq!(nlen, !(len as u16));
            out.extend_from_slice(&zlib[at + 5..at + 5 + len]);
            at += 5 + len;
            if header & 1 == 1 {
                break;
            }
        }
        let checksum = u32::from_be_bytes(zlib[at..at + 4].try_into().unwrap());
        assert_eq!(checksum, adler32(&out));
        assert_eq!(at + 4, zlib.len());
        out
    }

    #[test]
    fn crc32_matches_reference_values() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn adler32_matches_reference_values() {
        assert_eq!(adler32(b""), 1);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn encode_writes_signature_header_and_end() {
        let png = encode_rgba(3, 2, &[7; 24]);
        assert_eq!(png[..8], SIGNATURE);
        let list = chunks(&png);
        assert_eq!(list.first().unwrap().0, "IHDR");
        assert_eq!(list.last().unwrap(), &("IEND".to_string(), vec![]));
        let ihdr = &list[0].1;
        assert_eq!(ihdr.len(), 13);
        assert_eq!(u32::from_be_bytes(ihdr[0..4].try_into().unwrap()), 3);
        assert_eq!(u32::from_be_bytes(ihdr[4..8].try_into().unwrap()), 2);
        assert_eq!(&ihdr[8..], &[8, 6, 0, 0, 0]);
    }

    #[test]
    fn encode_stores_unfiltered_scanlines() {
        let rgba: Vec<u8> = (0..24).collect();
        let png = encode_rgba(3, 2, &rgba);
        let data: Vec<u8> = chunks(&png)
            .into_iter()
            .filter(|(kind, _)| kind == "IDAT")
            .flat_map(|(_, d)| d)
            .collect();
        let raw = inflate_stored(&data);
        let mut expected = vec![0];
        expected.extend_from_slice(&rgba[..12]);
        expected.push(0);
        expected.extend_from_slice(&rgba[12..]);
        assert_eq!(raw, expected);
    }

    #[test]
    fn encode_splits_large_images_into_many_stored_blocks() {
        let rgba = vec![200; 300 * 100 * 4];
        let png = encode_rgba(300, 100, &rgba);
        let data: Vec<u8> = chunks(&png)
            .into_iter()
            .filter(|(kind, _)| kind == "IDAT")
            .flat_map(|(_, d)| d)
            .collect();
        assert_eq!(inflate_stored(&data).len(), 100 * (1 + 300 * 4));
    }
}
