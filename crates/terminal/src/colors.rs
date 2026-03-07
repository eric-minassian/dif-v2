/// ANSI 256-color palette.
///
/// Indices 0–15:   Standard 16 colors (Apple System Colors theme).
/// Indices 16–231: 6×6×6 color cube per the xterm specification.
///                 Each component maps as: `0 → 0`, `1..=5 → 55 + 40 * n`.
/// Indices 232–255: 24-step grayscale ramp (`8 + 10 * n`).
pub(crate) const ANSI_COLORS: [(u8, u8, u8); 256] = {
    let mut table = [(0u8, 0u8, 0u8); 256];

    // Standard 16 colors (Apple System Colors theme)
    table[0] = (0x1a, 0x1a, 0x1a); // black
    table[1] = (0xcc, 0x37, 0x2e); // red
    table[2] = (0x26, 0xa4, 0x39); // green
    table[3] = (0xcd, 0xac, 0x08); // yellow
    table[4] = (0x08, 0x69, 0xcb); // blue
    table[5] = (0x96, 0x47, 0xbf); // magenta
    table[6] = (0x47, 0x9e, 0xc2); // cyan
    table[7] = (0x98, 0x98, 0x9d); // white
    table[8] = (0x46, 0x46, 0x46); // bright black
    table[9] = (0xff, 0x45, 0x3a); // bright red
    table[10] = (0x32, 0xd7, 0x4b); // bright green
    table[11] = (0xff, 0xd6, 0x0a); // bright yellow
    table[12] = (0x0a, 0x84, 0xff); // bright blue
    table[13] = (0xbf, 0x5a, 0xf2); // bright magenta
    table[14] = (0x76, 0xd6, 0xff); // bright cyan
    table[15] = (0xff, 0xff, 0xff); // bright white

    // 6×6×6 color cube (indices 16..=231)
    let mut i = 16usize;
    let mut ri = 0u8;
    while ri < 6 {
        let mut gi = 0u8;
        while gi < 6 {
            let mut bi = 0u8;
            while bi < 6 {
                let r = if ri == 0 { 0 } else { 55 + 40 * ri };
                let g = if gi == 0 { 0 } else { 55 + 40 * gi };
                let b = if bi == 0 { 0 } else { 55 + 40 * bi };
                table[i] = (r, g, b);
                i += 1;
                bi += 1;
            }
            gi += 1;
        }
        ri += 1;
    }

    // 24-step grayscale ramp (indices 232..=255)
    let mut j = 0u8;
    while j < 24 {
        let v = 8 + 10 * j;
        table[232 + j as usize] = (v, v, v);
        j += 1;
    }

    table
};
