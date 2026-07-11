//! Bink1 video decoder adapted from NihAV by Kostya Shishkov.
//! Original: https://git.nihav.org/nihav/ (nihav-rad/src/codecs/binkvid.rs)
//!
//! Self-contained port with no external dependencies. Only Bink1 decoder.

use std::f32::consts::{PI, SQRT_2};

// ---------------------------------------------------------------------------
// Section 1: Constants
// ---------------------------------------------------------------------------

const SKIP_BLOCK: u8 = 0;
const SCALED_BLOCK: u8 = 1;
const MOTION_BLOCK: u8 = 2;
const RUN_BLOCK: u8 = 3;
const RESIDUE_BLOCK: u8 = 4;
const INTRA_BLOCK: u8 = 5;
const FILL_BLOCK: u8 = 6;
const INTER_BLOCK: u8 = 7;
const PATTERN_BLOCK: u8 = 8;
const RAW_BLOCK: u8 = 9;

const DC_START_BITS: u8 = 11;
const BLOCK_TYPE_RUNS: [usize; 4] = [4, 8, 12, 32];
const BINK_FLAG_ALPHA: u32 = 0x0010_0000;
const BINK_FLAG_GRAY: u32 = 0x0002_0000;
const TABLE_FILL: u32 = 0xFFFF_FFFF;

// ---------------------------------------------------------------------------
// Section 2: Static tables (from binkvid.rs lines 1309-1838)
// ---------------------------------------------------------------------------

const BINK_SCAN: [usize; 64] = [
    0, 1, 8, 9, 2, 3, 10, 11, 4, 5, 12, 13, 6, 7, 14, 15, 20, 21, 28, 29, 22, 23, 30, 31, 16, 17,
    24, 25, 32, 33, 40, 41, 34, 35, 42, 43, 48, 49, 56, 57, 50, 51, 58, 59, 18, 19, 26, 27, 36, 37,
    44, 45, 38, 39, 46, 47, 52, 53, 60, 61, 54, 55, 62, 63,
];

const BINK_TREE_CODES: [[u8; 16]; 16] = [
    [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F,
    ],
    [
        0x00, 0x01, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0D, 0x0F, 0x13, 0x15, 0x17, 0x19, 0x1B, 0x1D,
        0x1F,
    ],
    [
        0x00, 0x02, 0x01, 0x09, 0x05, 0x15, 0x0D, 0x1D, 0x03, 0x13, 0x0B, 0x1B, 0x07, 0x17, 0x0F,
        0x1F,
    ],
    [
        0x00, 0x02, 0x06, 0x01, 0x09, 0x05, 0x0D, 0x1D, 0x03, 0x13, 0x0B, 0x1B, 0x07, 0x17, 0x0F,
        0x1F,
    ],
    [
        0x00, 0x04, 0x02, 0x06, 0x01, 0x09, 0x05, 0x0D, 0x03, 0x13, 0x0B, 0x1B, 0x07, 0x17, 0x0F,
        0x1F,
    ],
    [
        0x00, 0x04, 0x02, 0x0A, 0x06, 0x0E, 0x01, 0x09, 0x05, 0x0D, 0x03, 0x0B, 0x07, 0x17, 0x0F,
        0x1F,
    ],
    [
        0x00, 0x02, 0x0A, 0x06, 0x0E, 0x01, 0x09, 0x05, 0x0D, 0x03, 0x0B, 0x1B, 0x07, 0x17, 0x0F,
        0x1F,
    ],
    [
        0x00, 0x01, 0x05, 0x03, 0x13, 0x0B, 0x1B, 0x3B, 0x07, 0x27, 0x17, 0x37, 0x0F, 0x2F, 0x1F,
        0x3F,
    ],
    [
        0x00, 0x01, 0x03, 0x13, 0x0B, 0x2B, 0x1B, 0x3B, 0x07, 0x27, 0x17, 0x37, 0x0F, 0x2F, 0x1F,
        0x3F,
    ],
    [
        0x00, 0x01, 0x05, 0x0D, 0x03, 0x13, 0x0B, 0x1B, 0x07, 0x27, 0x17, 0x37, 0x0F, 0x2F, 0x1F,
        0x3F,
    ],
    [
        0x00, 0x02, 0x01, 0x05, 0x0D, 0x03, 0x13, 0x0B, 0x1B, 0x07, 0x17, 0x37, 0x0F, 0x2F, 0x1F,
        0x3F,
    ],
    [
        0x00, 0x01, 0x09, 0x05, 0x0D, 0x03, 0x13, 0x0B, 0x1B, 0x07, 0x17, 0x37, 0x0F, 0x2F, 0x1F,
        0x3F,
    ],
    [
        0x00, 0x02, 0x01, 0x03, 0x13, 0x0B, 0x1B, 0x3B, 0x07, 0x27, 0x17, 0x37, 0x0F, 0x2F, 0x1F,
        0x3F,
    ],
    [
        0x00, 0x01, 0x05, 0x03, 0x07, 0x27, 0x17, 0x37, 0x0F, 0x4F, 0x2F, 0x6F, 0x1F, 0x5F, 0x3F,
        0x7F,
    ],
    [
        0x00, 0x01, 0x05, 0x03, 0x07, 0x17, 0x37, 0x77, 0x0F, 0x4F, 0x2F, 0x6F, 0x1F, 0x5F, 0x3F,
        0x7F,
    ],
    [
        0x00, 0x02, 0x01, 0x05, 0x03, 0x07, 0x27, 0x17, 0x37, 0x0F, 0x2F, 0x6F, 0x1F, 0x5F, 0x3F,
        0x7F,
    ],
];

const BINK_TREE_BITS: [[u8; 16]; 16] = [
    [4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4],
    [1, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
    [2, 2, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
    [2, 3, 3, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
    [3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5],
    [3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5],
    [2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5],
    [1, 3, 3, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6],
    [1, 2, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6],
    [1, 3, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6],
    [2, 2, 3, 4, 4, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6],
    [1, 4, 4, 4, 4, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6],
    [2, 2, 2, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6],
    [1, 3, 3, 3, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7],
    [1, 3, 3, 3, 5, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7],
    [2, 2, 3, 3, 3, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7],
];

const BINK_PATTERNS: [[u8; 64]; 16] = [
    [
        0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x39, 0x31, 0x29, 0x21, 0x19, 0x11, 0x09,
        0x01, 0x02, 0x0A, 0x12, 0x1A, 0x22, 0x2A, 0x32, 0x3A, 0x3B, 0x33, 0x2B, 0x23, 0x1B, 0x13,
        0x0B, 0x03, 0x04, 0x0C, 0x14, 0x1C, 0x24, 0x2C, 0x34, 0x3C, 0x3D, 0x35, 0x2D, 0x25, 0x1D,
        0x15, 0x0D, 0x05, 0x06, 0x0E, 0x16, 0x1E, 0x26, 0x2E, 0x36, 0x3E, 0x3F, 0x37, 0x2F, 0x27,
        0x1F, 0x17, 0x0F, 0x07,
    ],
    [
        0x3B, 0x3A, 0x39, 0x38, 0x30, 0x31, 0x32, 0x33, 0x2B, 0x2A, 0x29, 0x28, 0x20, 0x21, 0x22,
        0x23, 0x1B, 0x1A, 0x19, 0x18, 0x10, 0x11, 0x12, 0x13, 0x0B, 0x0A, 0x09, 0x08, 0x00, 0x01,
        0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x0F, 0x0E, 0x0D, 0x0C, 0x14, 0x15, 0x16, 0x17, 0x1F,
        0x1E, 0x1D, 0x1C, 0x24, 0x25, 0x26, 0x27, 0x2F, 0x2E, 0x2D, 0x2C, 0x34, 0x35, 0x36, 0x37,
        0x3F, 0x3E, 0x3D, 0x3C,
    ],
    [
        0x19, 0x11, 0x12, 0x1A, 0x1B, 0x13, 0x0B, 0x03, 0x02, 0x0A, 0x09, 0x01, 0x00, 0x08, 0x10,
        0x18, 0x20, 0x28, 0x30, 0x38, 0x39, 0x31, 0x29, 0x2A, 0x32, 0x3A, 0x3B, 0x33, 0x2B, 0x23,
        0x22, 0x21, 0x1D, 0x15, 0x16, 0x1E, 0x1F, 0x17, 0x0F, 0x07, 0x06, 0x0E, 0x0D, 0x05, 0x04,
        0x0C, 0x14, 0x1C, 0x24, 0x2C, 0x34, 0x3C, 0x3D, 0x35, 0x2D, 0x2E, 0x36, 0x3E, 0x3F, 0x37,
        0x2F, 0x27, 0x26, 0x25,
    ],
    [
        0x03, 0x0B, 0x02, 0x0A, 0x01, 0x09, 0x00, 0x08, 0x10, 0x18, 0x11, 0x19, 0x12, 0x1A, 0x13,
        0x1B, 0x23, 0x2B, 0x22, 0x2A, 0x21, 0x29, 0x20, 0x28, 0x30, 0x38, 0x31, 0x39, 0x32, 0x3A,
        0x33, 0x3B, 0x3C, 0x34, 0x3D, 0x35, 0x3E, 0x36, 0x3F, 0x37, 0x2F, 0x27, 0x2E, 0x26, 0x2D,
        0x25, 0x2C, 0x24, 0x1C, 0x14, 0x1D, 0x15, 0x1E, 0x16, 0x1F, 0x17, 0x0F, 0x07, 0x0E, 0x06,
        0x0D, 0x05, 0x0C, 0x04,
    ],
    [
        0x18, 0x19, 0x10, 0x11, 0x08, 0x09, 0x00, 0x01, 0x02, 0x03, 0x0A, 0x0B, 0x12, 0x13, 0x1A,
        0x1B, 0x1C, 0x1D, 0x14, 0x15, 0x0C, 0x0D, 0x04, 0x05, 0x06, 0x07, 0x0E, 0x0F, 0x16, 0x17,
        0x1E, 0x1F, 0x27, 0x26, 0x2F, 0x2E, 0x37, 0x36, 0x3F, 0x3E, 0x3D, 0x3C, 0x35, 0x34, 0x2D,
        0x2C, 0x25, 0x24, 0x23, 0x22, 0x2B, 0x2A, 0x33, 0x32, 0x3B, 0x3A, 0x39, 0x38, 0x31, 0x30,
        0x29, 0x28, 0x21, 0x20,
    ],
    [
        0x00, 0x01, 0x02, 0x03, 0x08, 0x09, 0x0A, 0x0B, 0x10, 0x11, 0x12, 0x13, 0x18, 0x19, 0x1A,
        0x1B, 0x20, 0x21, 0x22, 0x23, 0x28, 0x29, 0x2A, 0x2B, 0x30, 0x31, 0x32, 0x33, 0x38, 0x39,
        0x3A, 0x3B, 0x04, 0x05, 0x06, 0x07, 0x0C, 0x0D, 0x0E, 0x0F, 0x14, 0x15, 0x16, 0x17, 0x1C,
        0x1D, 0x1E, 0x1F, 0x24, 0x25, 0x26, 0x27, 0x2C, 0x2D, 0x2E, 0x2F, 0x34, 0x35, 0x36, 0x37,
        0x3C, 0x3D, 0x3E, 0x3F,
    ],
    [
        0x06, 0x07, 0x0F, 0x0E, 0x0D, 0x05, 0x0C, 0x04, 0x03, 0x0B, 0x02, 0x0A, 0x09, 0x01, 0x00,
        0x08, 0x10, 0x18, 0x11, 0x19, 0x12, 0x1A, 0x13, 0x1B, 0x14, 0x1C, 0x15, 0x1D, 0x16, 0x1E,
        0x17, 0x1F, 0x27, 0x2F, 0x26, 0x2E, 0x25, 0x2D, 0x24, 0x2C, 0x23, 0x2B, 0x22, 0x2A, 0x21,
        0x29, 0x20, 0x28, 0x31, 0x30, 0x38, 0x39, 0x3A, 0x32, 0x3B, 0x33, 0x3C, 0x34, 0x3D, 0x35,
        0x36, 0x37, 0x3F, 0x3E,
    ],
    [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x0F, 0x0E, 0x0D, 0x0C, 0x0B, 0x0A, 0x09,
        0x08, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x1F, 0x1E, 0x1D, 0x1C, 0x1B, 0x1A,
        0x19, 0x18, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2F, 0x2E, 0x2D, 0x2C, 0x2B,
        0x2A, 0x29, 0x28, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x3F, 0x3E, 0x3D, 0x3C,
        0x3B, 0x3A, 0x39, 0x38,
    ],
    [
        0x00, 0x08, 0x09, 0x01, 0x02, 0x03, 0x0B, 0x0A, 0x12, 0x13, 0x1B, 0x1A, 0x19, 0x11, 0x10,
        0x18, 0x20, 0x28, 0x29, 0x21, 0x22, 0x23, 0x2B, 0x2A, 0x32, 0x31, 0x30, 0x38, 0x39, 0x3A,
        0x3B, 0x33, 0x34, 0x3C, 0x3D, 0x3E, 0x3F, 0x37, 0x36, 0x35, 0x2D, 0x2C, 0x24, 0x25, 0x26,
        0x2E, 0x2F, 0x27, 0x1F, 0x17, 0x16, 0x1E, 0x1D, 0x1C, 0x14, 0x15, 0x0D, 0x0C, 0x04, 0x05,
        0x06, 0x0E, 0x0F, 0x07,
    ],
    [
        0x18, 0x19, 0x10, 0x11, 0x08, 0x09, 0x00, 0x01, 0x02, 0x03, 0x0A, 0x0B, 0x12, 0x13, 0x1A,
        0x1B, 0x1C, 0x1D, 0x14, 0x15, 0x0C, 0x0D, 0x04, 0x05, 0x06, 0x07, 0x0E, 0x0F, 0x16, 0x17,
        0x1E, 0x1F, 0x26, 0x27, 0x2E, 0x2F, 0x36, 0x37, 0x3E, 0x3F, 0x3C, 0x3D, 0x34, 0x35, 0x2C,
        0x2D, 0x24, 0x25, 0x22, 0x23, 0x2A, 0x2B, 0x32, 0x33, 0x3A, 0x3B, 0x38, 0x39, 0x30, 0x31,
        0x28, 0x29, 0x20, 0x21,
    ],
    [
        0x00, 0x08, 0x01, 0x09, 0x02, 0x0A, 0x03, 0x0B, 0x13, 0x1B, 0x12, 0x1A, 0x11, 0x19, 0x10,
        0x18, 0x20, 0x28, 0x21, 0x29, 0x22, 0x2A, 0x23, 0x2B, 0x33, 0x3B, 0x32, 0x3A, 0x31, 0x39,
        0x30, 0x38, 0x3C, 0x34, 0x3D, 0x35, 0x3E, 0x36, 0x3F, 0x37, 0x2F, 0x27, 0x2E, 0x26, 0x2D,
        0x25, 0x2C, 0x24, 0x1F, 0x17, 0x1E, 0x16, 0x1D, 0x15, 0x1C, 0x14, 0x0C, 0x04, 0x0D, 0x05,
        0x0E, 0x06, 0x0F, 0x07,
    ],
    [
        0x00, 0x08, 0x10, 0x18, 0x19, 0x1A, 0x1B, 0x13, 0x0B, 0x03, 0x02, 0x01, 0x09, 0x11, 0x12,
        0x0A, 0x04, 0x0C, 0x14, 0x1C, 0x1D, 0x1E, 0x1F, 0x17, 0x0F, 0x07, 0x06, 0x05, 0x0D, 0x15,
        0x16, 0x0E, 0x24, 0x2C, 0x34, 0x3C, 0x3D, 0x3E, 0x3F, 0x37, 0x2F, 0x27, 0x26, 0x25, 0x2D,
        0x35, 0x36, 0x2E, 0x20, 0x28, 0x30, 0x38, 0x39, 0x3A, 0x3B, 0x33, 0x2B, 0x23, 0x22, 0x21,
        0x29, 0x31, 0x32, 0x2A,
    ],
    [
        0x00, 0x08, 0x09, 0x01, 0x02, 0x03, 0x0B, 0x0A, 0x13, 0x1B, 0x1A, 0x12, 0x11, 0x10, 0x18,
        0x19, 0x21, 0x20, 0x28, 0x29, 0x2A, 0x22, 0x23, 0x2B, 0x33, 0x3B, 0x3A, 0x32, 0x31, 0x39,
        0x38, 0x30, 0x34, 0x3C, 0x3D, 0x35, 0x36, 0x3E, 0x3F, 0x37, 0x2F, 0x27, 0x26, 0x2E, 0x2D,
        0x2C, 0x24, 0x25, 0x1D, 0x1C, 0x14, 0x15, 0x16, 0x1E, 0x1F, 0x17, 0x0E, 0x0F, 0x07, 0x06,
        0x05, 0x0D, 0x0C, 0x04,
    ],
    [
        0x18, 0x10, 0x08, 0x00, 0x01, 0x02, 0x03, 0x0B, 0x13, 0x1B, 0x1A, 0x19, 0x11, 0x0A, 0x09,
        0x12, 0x1C, 0x14, 0x0C, 0x04, 0x05, 0x06, 0x07, 0x0F, 0x17, 0x1F, 0x1E, 0x1D, 0x15, 0x0E,
        0x0D, 0x16, 0x3C, 0x34, 0x2C, 0x24, 0x25, 0x26, 0x27, 0x2F, 0x37, 0x3F, 0x3E, 0x3D, 0x35,
        0x2E, 0x2D, 0x36, 0x38, 0x30, 0x28, 0x20, 0x21, 0x22, 0x23, 0x2B, 0x33, 0x3B, 0x3A, 0x39,
        0x31, 0x2A, 0x29, 0x32,
    ],
    [
        0x00, 0x08, 0x09, 0x01, 0x02, 0x0A, 0x12, 0x11, 0x10, 0x18, 0x19, 0x1A, 0x1B, 0x13, 0x0B,
        0x03, 0x07, 0x06, 0x0E, 0x0F, 0x17, 0x16, 0x15, 0x0D, 0x05, 0x04, 0x0C, 0x14, 0x1C, 0x1D,
        0x1E, 0x1F, 0x3F, 0x3E, 0x36, 0x37, 0x2F, 0x2E, 0x2D, 0x35, 0x3D, 0x3C, 0x34, 0x2C, 0x24,
        0x25, 0x26, 0x27, 0x38, 0x30, 0x31, 0x39, 0x3A, 0x32, 0x2A, 0x29, 0x28, 0x20, 0x21, 0x22,
        0x23, 0x2B, 0x33, 0x3B,
    ],
    [
        0x00, 0x01, 0x08, 0x09, 0x10, 0x11, 0x18, 0x19, 0x20, 0x21, 0x28, 0x29, 0x30, 0x31, 0x38,
        0x39, 0x3A, 0x3B, 0x32, 0x33, 0x2A, 0x2B, 0x22, 0x23, 0x1A, 0x1B, 0x12, 0x13, 0x0A, 0x0B,
        0x02, 0x03, 0x04, 0x05, 0x0C, 0x0D, 0x14, 0x15, 0x1C, 0x1D, 0x24, 0x25, 0x2C, 0x2D, 0x34,
        0x35, 0x3C, 0x3D, 0x3E, 0x3F, 0x36, 0x37, 0x2E, 0x2F, 0x26, 0x27, 0x1E, 0x1F, 0x16, 0x17,
        0x0E, 0x0F, 0x06, 0x07,
    ],
];

// BINK_INTRA_QUANT and BINK_INTER_QUANT are loaded from the NihAV source verbatim.
// The `include!` macro pulls them from a separate data file to keep this file manageable.
include!("bink_quant_tables.rs");

const BINKB_RUN_BITS: [u8; 64] = [
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 4, 4, 4, 4, 4, 4, 4, 3, 3, 3, 3, 2, 2, 1, 0,
];

const BINKB_REF_INTRA_Q: [u8; 64] = [
    16, 16, 16, 19, 16, 19, 22, 22, 22, 22, 26, 24, 26, 22, 22, 27, 27, 27, 26, 26, 26, 29, 29, 29,
    27, 27, 27, 26, 34, 34, 34, 29, 29, 29, 27, 27, 37, 34, 34, 32, 32, 29, 29, 38, 37, 35, 35, 34,
    35, 40, 40, 40, 38, 38, 48, 48, 46, 46, 58, 56, 56, 69, 69, 83,
];

const BINKB_REF_INTER_Q: [u8; 64] = [
    16, 17, 17, 18, 18, 18, 19, 19, 19, 19, 20, 20, 20, 20, 20, 21, 21, 21, 21, 21, 21, 22, 22, 22,
    22, 22, 22, 22, 23, 23, 23, 23, 23, 23, 23, 23, 24, 24, 24, 25, 24, 24, 24, 25, 26, 26, 26, 26,
    25, 27, 27, 27, 27, 27, 28, 28, 28, 28, 30, 30, 30, 31, 31, 33,
];

const BINKB_REF_QUANTS: [(u8, u8); 16] = [
    (1, 1),
    (4, 3),
    (5, 3),
    (2, 1),
    (7, 3),
    (8, 3),
    (3, 1),
    (7, 2),
    (4, 1),
    (9, 2),
    (5, 1),
    (6, 1),
    (7, 1),
    (8, 1),
    (9, 1),
    (10, 1),
];

// ---------------------------------------------------------------------------
// Section 3: BitReader — LE (little-endian / LSB-first) bitstream reader
// Adapted from NihAV nihav-core/src/io/bitreader.rs, LE mode only.
// ---------------------------------------------------------------------------

pub struct BitReader<'a> {
    cache: u64,
    bits: u8,
    pos: usize,
    src: &'a [u8],
}

impl<'a> BitReader<'a> {
    pub fn new(src: &'a [u8]) -> Self {
        let mut br = Self {
            cache: 0,
            bits: 0,
            pos: 0,
            src,
        };
        br.refill();
        br
    }

    pub fn tell(&self) -> u32 {
        (self.pos as u32 * 8).saturating_sub(self.bits as u32)
    }

    fn refill(&mut self) {
        while self.bits <= 32 {
            if self.pos + 4 <= self.src.len() {
                let nw = u32::from(self.src[self.pos])
                    | (u32::from(self.src[self.pos + 1]) << 8)
                    | (u32::from(self.src[self.pos + 2]) << 16)
                    | (u32::from(self.src[self.pos + 3]) << 24);
                self.cache |= u64::from(nw) << self.bits;
                self.pos += 4;
                self.bits += 32;
            } else {
                while self.pos < self.src.len() && self.bits <= 56 {
                    self.cache |= u64::from(self.src[self.pos]) << self.bits;
                    self.pos += 1;
                    self.bits += 8;
                }
                break;
            }
        }
    }

    pub fn read(&mut self, n: u8) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err("bink: too many bits requested".to_string());
        }
        if self.bits < n {
            self.refill();
            if self.bits < n {
                return Err("bink: unexpected end of bitstream".to_string());
            }
        }
        let res = ((1u64 << n) - 1) & self.cache;
        self.cache >>= n;
        self.bits -= n;
        Ok(res as u32)
    }

    pub fn read_bool(&mut self) -> Result<bool, String> {
        Ok(self.read(1)? != 0)
    }

    pub fn peek(&mut self, n: u8) -> u32 {
        if n > 32 {
            return 0;
        }
        if self.bits < n {
            self.refill();
        }
        (((1u64 << n).wrapping_sub(1)) & self.cache) as u32
    }

    pub fn skip(&mut self, n: u32) -> Result<(), String> {
        if (n as u8) <= self.bits {
            self.cache >>= n as u8;
            self.bits -= n as u8;
            return Ok(());
        }
        let mut remaining = n - self.bits as u32;
        self.cache = 0;
        self.bits = 0;
        self.pos += ((remaining / 32) * 4) as usize;
        remaining &= 0x1F;
        self.refill();
        if remaining > 0 {
            if self.bits < remaining as u8 {
                return Err("bink: skip past end of bitstream".to_string());
            }
            self.cache >>= remaining as u8;
            self.bits -= remaining as u8;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Section 4: HuffmanTable — LSB VLC lookup table for Bink tree codes
// Simplified from NihAV nihav-core/src/io/codebook.rs, LSB single-level only.
// ---------------------------------------------------------------------------

struct HuffmanTable {
    /// Each entry: `(symbol_value << 8) | code_length`, or TABLE_FILL if unset.
    table: Vec<u32>,
    lut_bits: u8,
}

impl HuffmanTable {
    fn build(codes: &[u8], code_bits: &[u8]) -> Result<Self, String> {
        let max_bits = *code_bits.iter().max().unwrap_or(&0);
        if max_bits == 0 || max_bits > 12 {
            return Err("bink: invalid Huffman table bit lengths".to_string());
        }
        let lut_bits = max_bits;
        let lut_size = 1 << lut_bits;
        let mut table = vec![TABLE_FILL; lut_size];

        for (sym, (&code, &bits)) in codes.iter().zip(code_bits.iter()).enumerate() {
            if bits == 0 {
                continue;
            }
            let entry = (sym as u32) << 8 | (bits as u32);
            let fill_len = lut_bits - bits;
            let fill_size = 1 << fill_len;
            for j in 0..fill_size {
                let idx = (code as u32 + (j << bits as u32)) as usize;
                if idx < lut_size {
                    table[idx] = entry;
                }
            }
        }

        Ok(Self { table, lut_bits })
    }

    fn read_symbol(&self, br: &mut BitReader) -> Result<u32, String> {
        let peeked = br.peek(self.lut_bits);
        let entry = self
            .table
            .get(peeked as usize)
            .copied()
            .unwrap_or(TABLE_FILL);
        if entry == TABLE_FILL {
            return Err("bink: invalid Huffman code".to_string());
        }
        let sym = entry >> 8;
        let code_len = (entry & 0xFF) as u8;
        br.read(code_len)?;
        Ok(sym)
    }
}

// ---------------------------------------------------------------------------
// Section 5: BinkTrees — 16 HuffmanTables built from BINK_TREE_CODES / BITS
// ---------------------------------------------------------------------------

struct BinkTrees {
    tables: [HuffmanTable; 16],
}

impl Default for BinkTrees {
    fn default() -> Self {
        Self {
            tables: std::array::from_fn(|i| {
                HuffmanTable::build(&BINK_TREE_CODES[i], &BINK_TREE_BITS[i])
                    .expect("Bink tree Huffman tables should be valid")
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Section 6: Tree — symbol permutation tree for bundle decoding
// Ported from NihAV binkvid.rs lines 19-86
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Copy)]
struct Tree {
    id: usize,
    syms: [u8; 16],
}

impl Tree {
    fn read_desc(&mut self, br: &mut BitReader) -> Result<(), String> {
        self.id = br.read(4)? as usize;
        if self.id == 0 {
            for i in 0..16 {
                self.syms[i] = i as u8;
            }
        } else if br.read_bool()? {
            let len = br.read(3)? as usize;
            let mut present: [bool; 16] = [false; 16];
            for i in 0..=len {
                self.syms[i] = br.read(4)? as u8;
                present[self.syms[i] as usize] = true;
            }
            let mut idx = len + 1;
            for i in 0..16 {
                if present[i] {
                    continue;
                }
                self.syms[idx] = i as u8;
                idx += 1;
            }
        } else {
            let len = br.read(2)? as usize;
            let mut syms: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
            let mut tmp: [u8; 16] = [0; 16];
            for bits in 0..=len {
                let size = 1 << bits;
                for arr in syms.chunks_mut(size * 2) {
                    let mut ptr0 = 0;
                    let mut ptr1 = size;
                    let mut optr = 0;
                    while (ptr0 < size) && (ptr1 < size * 2) {
                        if !br.read_bool()? {
                            tmp[optr] = arr[ptr0];
                            ptr0 += 1;
                        } else {
                            tmp[optr] = arr[ptr1];
                            ptr1 += 1;
                        }
                        optr += 1;
                    }
                    while ptr0 < size {
                        tmp[optr] = arr[ptr0];
                        ptr0 += 1;
                        optr += 1;
                    }
                    while ptr1 < size * 2 {
                        tmp[optr] = arr[ptr1];
                        ptr1 += 1;
                        optr += 1;
                    }
                    arr.copy_from_slice(&tmp[0..size * 2]);
                }
            }
            self.syms = syms;
        }
        Ok(())
    }

    fn read_sym(&self, br: &mut BitReader, trees: &BinkTrees) -> Result<u8, String> {
        let idx = trees.tables[self.id].read_symbol(br)?;
        Ok(self.syms[idx as usize])
    }
}

// ---------------------------------------------------------------------------
// Section 7: Bundle<T> — generic data bundle with tree-based entropy coding
// Ported from NihAV binkvid.rs lines 88-378
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Bundle<T: Copy> {
    tree: Tree,
    data: Vec<T>,
    dec_pos: usize,
    read_pos: usize,
    bits: u8,
}

impl<T: Copy> Bundle<T> {
    fn binkb_reset(&mut self, bits: u8) {
        self.bits = bits;
        self.dec_pos = 0;
        self.read_pos = 0;
    }

    fn reset(&mut self) {
        self.dec_pos = 0;
        self.read_pos = 0;
    }

    fn read_desc(&mut self, br: &mut BitReader) -> Result<(), String> {
        self.dec_pos = 0;
        self.read_pos = 0;
        self.tree.read_desc(br)?;
        Ok(())
    }

    fn read_len(&mut self, br: &mut BitReader) -> Result<usize, String> {
        if self.read_pos < self.dec_pos {
            return Ok(0);
        }
        let len = br.read(self.bits)? as usize;
        if len == 0 {
            self.dec_pos = self.data.len();
            self.read_pos = self.data.len() - 1;
        }
        Ok(len)
    }

    fn read_len_binkb(&mut self, br: &mut BitReader) -> Result<usize, String> {
        if self.read_pos < self.dec_pos {
            return Ok(0);
        }
        let len = br.read(13)? as usize;
        if len == 0 {
            self.dec_pos = self.data.len();
            self.read_pos = self.data.len() - 1;
        }
        Ok(len)
    }

    fn get_val(&mut self) -> Result<T, String> {
        if self.read_pos >= self.dec_pos {
            return Err("bink: bundle read past decoded position".to_string());
        }
        let val = self.data[self.read_pos];
        self.read_pos += 1;
        Ok(val)
    }
}

impl Bundle<u8> {
    fn read_binkb(&mut self, br: &mut BitReader) -> Result<(), String> {
        let len = self.read_len_binkb(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: binkb u8 read past buffer".to_string());
        }
        for i in 0..len {
            self.data[self.dec_pos + i] = br.read(self.bits)? as u8;
        }
        self.dec_pos += len;
        Ok(())
    }

    fn read_runs(&mut self, br: &mut BitReader, trees: &BinkTrees) -> Result<(), String> {
        let len = self.read_len(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: runs read past buffer".to_string());
        }
        if br.read_bool()? {
            let val = br.read(4)? as u8;
            for i in 0..len {
                self.data[self.dec_pos + i] = val;
            }
            self.dec_pos += len;
        } else {
            while self.dec_pos < end {
                self.data[self.dec_pos] = self.tree.read_sym(br, trees)?;
                self.dec_pos += 1;
            }
        }
        Ok(())
    }

    fn read_block_types(&mut self, br: &mut BitReader, trees: &BinkTrees) -> Result<(), String> {
        let len = self.read_len(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: block types read past buffer".to_string());
        }
        if br.read_bool()? {
            let val = br.read(4)? as u8;
            for i in 0..len {
                self.data[self.dec_pos + i] = val;
            }
            self.dec_pos += len;
        } else {
            let mut last = 0;
            while self.dec_pos < end {
                let val = self.tree.read_sym(br, trees)?;
                if val < 12 {
                    self.data[self.dec_pos] = val;
                    self.dec_pos += 1;
                    last = val;
                } else {
                    let run = BLOCK_TYPE_RUNS[(val - 12) as usize];
                    if self.dec_pos + run > end {
                        return Err("bink: block type run past buffer".to_string());
                    }
                    for i in 0..run {
                        self.data[self.dec_pos + i] = last;
                    }
                    self.dec_pos += run;
                }
            }
        }
        Ok(())
    }

    fn read_patterns(&mut self, br: &mut BitReader, trees: &BinkTrees) -> Result<(), String> {
        let len = self.read_len(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: patterns read past buffer".to_string());
        }
        for i in 0..len {
            let pat_lo = self.tree.read_sym(br, trees)?;
            let pat_hi = self.tree.read_sym(br, trees)?;
            self.data[self.dec_pos + i] = pat_lo | (pat_hi << 4);
        }
        self.dec_pos += len;
        Ok(())
    }

    fn cvt_color(lo: u8, hi: u8, new_bink: bool) -> u8 {
        let val = lo | (hi << 4);
        if !new_bink {
            let sign = ((val as i8) >> 7) as u8;
            ((val & 0x7F) ^ sign).wrapping_sub(sign) ^ 0x80
        } else {
            val
        }
    }

    fn read_colors(
        &mut self,
        br: &mut BitReader,
        trees: &BinkTrees,
        col_hi: &[Tree; 16],
        col_last: &mut u8,
        new_bink: bool,
    ) -> Result<(), String> {
        let len = self.read_len(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: colors read past buffer".to_string());
        }
        let mut last = *col_last;
        if br.read_bool()? {
            last = col_hi[last as usize].read_sym(br, trees)?;
            let lo = self.tree.read_sym(br, trees)?;
            let val = Self::cvt_color(lo, last, new_bink);
            for i in 0..len {
                self.data[self.dec_pos + i] = val;
            }
            self.dec_pos += len;
        } else {
            while self.dec_pos < end {
                last = col_hi[last as usize].read_sym(br, trees)?;
                let lo = self.tree.read_sym(br, trees)?;
                let val = Self::cvt_color(lo, last, new_bink);
                self.data[self.dec_pos] = val;
                self.dec_pos += 1;
            }
        }
        *col_last = last;
        Ok(())
    }
}

impl Bundle<i8> {
    fn read_binkb(&mut self, br: &mut BitReader) -> Result<(), String> {
        let len = self.read_len_binkb(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: binkb i8 read past buffer".to_string());
        }
        let bias = 1 << (self.bits - 1);
        for i in 0..len {
            self.data[self.dec_pos + i] = (br.read(self.bits)? as i8) - bias;
        }
        self.dec_pos += len;
        Ok(())
    }

    fn read_motion_values(&mut self, br: &mut BitReader, trees: &BinkTrees) -> Result<(), String> {
        let len = self.read_len(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: motion values read past buffer".to_string());
        }
        if br.read_bool()? {
            let mut val = br.read(4)? as i8;
            if val != 0 && br.read_bool()? {
                val = -val;
            }
            for i in 0..len {
                self.data[self.dec_pos + i] = val;
            }
            self.dec_pos += len;
        } else {
            while self.dec_pos < end {
                self.data[self.dec_pos] = self.tree.read_sym(br, trees)? as i8;
                if self.data[self.dec_pos] != 0 && br.read_bool()? {
                    self.data[self.dec_pos] = -self.data[self.dec_pos];
                }
                self.dec_pos += 1;
            }
        }
        Ok(())
    }
}

impl Bundle<u16> {
    fn read_binkb(&mut self, br: &mut BitReader) -> Result<(), String> {
        let len = self.read_len_binkb(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: binkb u16 read past buffer".to_string());
        }
        for i in 0..len {
            self.data[self.dec_pos + i] = br.read(self.bits)? as u16;
        }
        self.dec_pos += len;
        Ok(())
    }

    fn read_dcs(&mut self, br: &mut BitReader, start_bits: u8) -> Result<(), String> {
        let len = self.read_len(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: dcs u16 read past buffer".to_string());
        }
        let mut val = br.read(start_bits)? as u16;
        self.data[self.dec_pos] = val;
        self.dec_pos += 1;
        for i in (1..len).step_by(8) {
            let seg_len = (len - i).min(8);
            let bits = br.read(4)? as u8;
            if bits != 0 {
                for _ in 0..seg_len {
                    let diff = br.read(bits)? as u16;
                    let res = if diff != 0 && br.read_bool()? {
                        val.checked_sub(diff)
                    } else {
                        val.checked_add(diff)
                    };
                    if res.is_none() {
                        return Err("bink: dcs u16 overflow".to_string());
                    }
                    val = res.unwrap();
                    self.data[self.dec_pos] = val;
                    self.dec_pos += 1;
                }
            } else {
                for _ in 0..seg_len {
                    self.data[self.dec_pos] = val;
                    self.dec_pos += 1;
                }
            }
        }
        Ok(())
    }
}

impl Bundle<i16> {
    fn read_binkb(&mut self, br: &mut BitReader) -> Result<(), String> {
        let len = self.read_len_binkb(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: binkb i16 read past buffer".to_string());
        }
        let bias = 1 << (self.bits - 1);
        for i in 0..len {
            self.data[self.dec_pos + i] = (br.read(self.bits)? as i16) - bias;
        }
        self.dec_pos += len;
        Ok(())
    }

    fn read_dcs(&mut self, br: &mut BitReader, start_bits: u8) -> Result<(), String> {
        let len = self.read_len(br)?;
        if len == 0 {
            return Ok(());
        }
        let end = self.dec_pos + len;
        if end > self.data.len() {
            return Err("bink: dcs i16 read past buffer".to_string());
        }
        let mut val = br.read(start_bits - 1)? as i16;
        if val != 0 && br.read_bool()? {
            val = -val;
        }
        self.data[self.dec_pos] = val;
        self.dec_pos += 1;
        for i in (1..len).step_by(8) {
            let seg_len = (len - i).min(8);
            let bits = br.read(4)? as u8;
            if bits != 0 {
                for _ in 0..seg_len {
                    let mut diff = br.read(bits)? as i16;
                    if diff != 0 && br.read_bool()? {
                        diff = -diff;
                    }
                    let res = val.checked_add(diff);
                    if res.is_none() {
                        return Err("bink: dcs i16 overflow".to_string());
                    }
                    val = res.unwrap();
                    self.data[self.dec_pos] = val;
                    self.dec_pos += 1;
                }
            } else {
                for _ in 0..seg_len {
                    self.data[self.dec_pos] = val;
                    self.dec_pos += 1;
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Section 8: IDCT — inverse discrete cosine transform
// Ported from NihAV binkvid.rs lines 400-434 (constants+macro), 602-706 (functions)
// ---------------------------------------------------------------------------

const A1: i32 = 2896;
const A2: i32 = 2217;
const A3: i32 = 3784;
const A4: i32 = -5352;

macro_rules! idct {
    ($src:expr, $sstep:expr, $dst:expr, $dstep:expr, $off:expr, $bias:expr, $shift:expr) => {
        let a0 = $src[$off + 0 * $sstep] + $src[$off + 4 * $sstep];
        let a1 = $src[$off + 0 * $sstep] - $src[$off + 4 * $sstep];
        let a2 = $src[$off + 2 * $sstep] + $src[$off + 6 * $sstep];
        let a3 = A1.wrapping_mul($src[$off + 2 * $sstep] - $src[$off + 6 * $sstep]) >> 11;
        let a4 = $src[$off + 5 * $sstep] + $src[$off + 3 * $sstep];
        let a5 = $src[$off + 5 * $sstep] - $src[$off + 3 * $sstep];
        let a6 = $src[$off + 1 * $sstep] + $src[$off + 7 * $sstep];
        let a7 = $src[$off + 1 * $sstep] - $src[$off + 7 * $sstep];
        let b0 = a4 + a6;
        let b1 = A3.wrapping_mul(a5 + a7) >> 11;
        let b2 = (A4.wrapping_mul(a5) >> 11) - b0 + b1;
        let b3 = (A1.wrapping_mul(a6 - a4) >> 11) - b2;
        let b4 = (A2.wrapping_mul(a7) >> 11) + b3 - b1;
        let c0 = a0 + a2;
        let c1 = a0 - a2;
        let c2 = a1 + (a3 - a2);
        let c3 = a1 - (a3 - a2);
        $dst[$off + 0 * $dstep] = (c0 + b0 + $bias) >> $shift;
        $dst[$off + 1 * $dstep] = (c2 + b2 + $bias) >> $shift;
        $dst[$off + 2 * $dstep] = (c3 + b3 + $bias) >> $shift;
        $dst[$off + 3 * $dstep] = (c1 - b4 + $bias) >> $shift;
        $dst[$off + 4 * $dstep] = (c1 + b4 + $bias) >> $shift;
        $dst[$off + 5 * $dstep] = (c3 - b3 + $bias) >> $shift;
        $dst[$off + 6 * $dstep] = (c2 - b2 + $bias) >> $shift;
        $dst[$off + 7 * $dstep] = (c0 - b0 + $bias) >> $shift;
    };
}

#[allow(clippy::identity_op)]
fn put_block(block: &[u8; 64], dst: &mut [u8], mut off: usize, stride: usize, scaled: bool) {
    if !scaled {
        for src in block.chunks_exact(8) {
            dst[off..off + 8].copy_from_slice(src);
            off += stride;
        }
    } else {
        for src in block.chunks_exact(8) {
            for i in 0..8 {
                dst[off + i * 2 + 0] = src[i];
                dst[off + i * 2 + 1] = src[i];
            }
            off += stride;
            for i in 0..8 {
                dst[off + i * 2 + 0] = src[i];
                dst[off + i * 2 + 1] = src[i];
            }
            off += stride;
        }
    }
}

fn add_block(coeffs: &[i32; 64], dst: &mut [u8], mut off: usize, stride: usize) {
    for src in coeffs.chunks_exact(8) {
        for i in 0..8 {
            let v = (dst[off + i] as i32) + src[i];
            dst[off + i] = v as u8;
        }
        off += stride;
    }
}

fn idct_put(coeffs: &[i32; 64], dst: &mut [u8], mut off: usize, stride: usize) {
    let mut tmp: [i32; 64] = [0; 64];
    let mut row: [i32; 8] = [0; 8];
    for i in 0..8 {
        idct!(coeffs, 8, tmp, 8, i, 0, 0);
    }
    for srow in tmp.chunks_exact(8) {
        idct!(srow, 1, row, 1, 0, 0x7F, 8);
        for i in 0..8 {
            dst[off + i] = row[i] as u8;
        }
        off += stride;
    }
}

fn idct_add(coeffs: &[i32; 64], dst: &mut [u8], mut off: usize, stride: usize) {
    let mut tmp: [i32; 64] = [0; 64];
    let mut row: [i32; 8] = [0; 8];
    for i in 0..8 {
        idct!(coeffs, 8, tmp, 8, i, 0, 0);
    }
    for srow in tmp.chunks_exact(8) {
        idct!(srow, 1, row, 1, 0, 0x7F, 8);
        for i in 0..8 {
            let v = (dst[off + i] as i32) + row[i];
            dst[off + i] = v as u8;
        }
        off += stride;
    }
}

// ---------------------------------------------------------------------------
// Section 9: QuantMats — quantization matrices for Bink version 'b'
// Ported from NihAV binkvid.rs lines 436-473
// ---------------------------------------------------------------------------

struct QuantMats {
    intra_qmat: [[i32; 64]; 16],
    inter_qmat: [[i32; 64]; 16],
}

impl QuantMats {
    fn calc_binkb_quants(&mut self) {
        let mut inv_scan: [usize; 64] = [0; 64];
        let mut mod_mat: [f32; 64] = [0.0; 64];
        let base = PI / 16.0;

        for i in 0..64 {
            inv_scan[BINK_SCAN[i]] = i;
        }

        for j in 0..8 {
            let j_scale = if (j != 0) && (j != 4) {
                (base * (j as f32)).cos() * SQRT_2
            } else {
                1.0
            };
            for i in 0..8 {
                let i_scale = if (i != 0) && (i != 4) {
                    (base * (i as f32)).cos() * SQRT_2
                } else {
                    1.0
                };
                mod_mat[i + j * 8] = i_scale * j_scale;
            }
        }

        for q in 0..16 {
            let (num, den) = BINKB_REF_QUANTS[q];
            let quant = (num as f32) * ((1 << 12) as f32) / (den as f32);
            for c in 0..64 {
                let idx = inv_scan[c];
                self.intra_qmat[q][idx] =
                    ((BINKB_REF_INTRA_Q[c] as f32) * mod_mat[c] * quant) as i32;
                self.inter_qmat[q][idx] =
                    ((BINKB_REF_INTER_Q[c] as f32) * mod_mat[c] * quant) as i32;
            }
        }
    }
}

impl Default for QuantMats {
    fn default() -> Self {
        Self {
            intra_qmat: [[0; 64]; 16],
            inter_qmat: [[0; 64]; 16],
        }
    }
}

// ---------------------------------------------------------------------------
// Section 10: DCT coefficient reader — split-list algorithm
// Ported from NihAV binkvid.rs lines 952-1048
// ---------------------------------------------------------------------------

fn get_coef(br: &mut BitReader, bits1: u8) -> Result<i32, String> {
    if bits1 == 1 {
        Ok(if br.read_bool()? { -1 } else { 1 })
    } else {
        let bits = bits1 - 1;
        let val = (br.read(bits)? as i32) | (1 << bits);
        if br.read_bool()? {
            Ok(-val)
        } else {
            Ok(val)
        }
    }
}

#[allow(clippy::identity_op)]
fn read_dct_coefficients(
    br: &mut BitReader,
    block: &mut [i32; 64],
    scan: &[usize; 64],
    quant_matrices: &[[i32; 64]; 16],
    q: Option<usize>,
) -> Result<(), String> {
    let mut coef_list: [i32; 128] = [0; 128];
    let mut mode_list: [u8; 128] = [0; 128];
    let mut list_start = 64;
    let mut list_end = 64;
    let mut coef_idx: [usize; 64] = [0; 64];
    let mut coef_count = 0;

    coef_list[list_end] = 4;
    mode_list[list_end] = 0;
    list_end += 1;
    coef_list[list_end] = 24;
    mode_list[list_end] = 0;
    list_end += 1;
    coef_list[list_end] = 44;
    mode_list[list_end] = 0;
    list_end += 1;
    coef_list[list_end] = 1;
    mode_list[list_end] = 3;
    list_end += 1;
    coef_list[list_end] = 2;
    mode_list[list_end] = 3;
    list_end += 1;
    coef_list[list_end] = 3;
    mode_list[list_end] = 3;
    list_end += 1;

    let mut bits1 = br.read(4)? as u8;
    while bits1 >= 1 {
        let mut list_pos = list_start;
        while list_pos < list_end {
            let ccoef = coef_list[list_pos];
            let mode = mode_list[list_pos];
            if (mode == 0 && ccoef == 0) || !br.read_bool()? {
                list_pos += 1;
                continue;
            }
            match mode {
                0 | 2 => {
                    if mode == 0 {
                        coef_list[list_pos] = ccoef + 4;
                        mode_list[list_pos] = 1;
                    } else {
                        coef_list[list_pos] = 0;
                        mode_list[list_pos] = 0;
                        list_pos += 1;
                    }
                    for i in 0..4u32 {
                        if br.read_bool()? {
                            list_start -= 1;
                            coef_list[list_start] = ccoef + i as i32;
                            mode_list[list_start] = 3;
                        } else {
                            let idx = (ccoef + i as i32) as usize;
                            block[scan[idx]] = get_coef(br, bits1)?;
                            coef_idx[coef_count] = idx;
                            coef_count += 1;
                        }
                    }
                }
                1 => {
                    mode_list[list_pos] = 2;
                    for i in 0..3u32 {
                        coef_list[list_end] = ccoef + i as i32 * 4 + 4;
                        mode_list[list_end] = 2;
                        list_end += 1;
                    }
                }
                3 => {
                    let idx = ccoef as usize;
                    block[scan[idx]] = get_coef(br, bits1)?;
                    coef_idx[coef_count] = idx;
                    coef_count += 1;
                    coef_list[list_pos] = 0;
                    mode_list[list_pos] = 0;
                    list_pos += 1;
                }
                _ => unreachable!(),
            };
        }
        bits1 -= 1;
    }

    let q_index = if let Some(qidx) = q {
        qidx
    } else {
        br.read(4)? as usize
    };
    let qmat = &quant_matrices[q_index];
    block[0] = block[0].wrapping_mul(qmat[0]) >> 11;
    for idx in coef_idx.iter().take(coef_count) {
        block[scan[*idx]] = block[scan[*idx]].wrapping_mul(qmat[*idx]) >> 11;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Section 11: Residue reader — split-list residue decoding
// Ported from NihAV binkvid.rs lines 1050-1141
// ---------------------------------------------------------------------------

fn read_residue(
    br: &mut BitReader,
    block: &mut [i32; 64],
    mut masks_count: usize,
) -> Result<(), String> {
    let mut coef_list: [i32; 128] = [0; 128];
    let mut mode_list: [u8; 128] = [0; 128];
    let mut list_start = 64;
    let mut list_end = 64;
    let mut nz_coef_idx: [usize; 64] = [0; 64];
    let mut nz_coef_count = 0;

    coef_list[list_end] = 4;
    mode_list[list_end] = 0;
    list_end += 1;
    coef_list[list_end] = 24;
    mode_list[list_end] = 0;
    list_end += 1;
    coef_list[list_end] = 44;
    mode_list[list_end] = 0;
    list_end += 1;
    coef_list[list_end] = 0;
    mode_list[list_end] = 2;
    list_end += 1;

    let mut mask = 1i32 << br.read(3)?;
    while mask > 0 {
        for i in 0..nz_coef_count {
            if !br.read_bool()? {
                continue;
            }
            let idx = nz_coef_idx[i];
            if block[idx] < 0 {
                block[idx] -= mask;
            } else {
                block[idx] += mask;
            }
            if masks_count == 0 {
                return Ok(());
            }
            masks_count -= 1;
        }
        let mut list_pos = list_start;
        while list_pos < list_end {
            let ccoef = coef_list[list_pos];
            let mode = mode_list[list_pos];
            if (mode == 0 && ccoef == 0) || !br.read_bool()? {
                list_pos += 1;
                continue;
            }
            match mode {
                0 | 2 => {
                    if mode == 0 {
                        coef_list[list_pos] = ccoef + 4;
                        mode_list[list_pos] = 1;
                    } else {
                        coef_list[list_pos] = 0;
                        mode_list[list_pos] = 0;
                        list_pos += 1;
                    }
                    for i in 0..4u32 {
                        if br.read_bool()? {
                            list_start -= 1;
                            coef_list[list_start] = ccoef + i as i32;
                            mode_list[list_start] = 3;
                        } else {
                            let idx = (ccoef + i as i32) as usize;
                            nz_coef_idx[nz_coef_count] = BINK_SCAN[idx];
                            nz_coef_count += 1;
                            block[BINK_SCAN[idx]] = if br.read_bool()? { -mask } else { mask };
                            if masks_count == 0 {
                                return Ok(());
                            }
                            masks_count -= 1;
                        }
                    }
                }
                1 => {
                    mode_list[list_pos] = 2;
                    for i in 0..3u32 {
                        coef_list[list_end] = ccoef + i as i32 * 4 + 4;
                        mode_list[list_end] = 2;
                        list_end += 1;
                    }
                }
                3 => {
                    let idx = ccoef as usize;
                    nz_coef_idx[nz_coef_count] = BINK_SCAN[idx];
                    nz_coef_count += 1;
                    block[BINK_SCAN[idx]] = if br.read_bool()? { -mask } else { mask };
                    coef_list[list_pos] = 0;
                    mode_list[list_pos] = 0;
                    list_pos += 1;
                    if masks_count == 0 {
                        return Ok(());
                    }
                    masks_count -= 1;
                }
                _ => unreachable!(),
            };
        }
        mask >>= 1;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Section 12: BinkVideoDecoder — main decoder struct and frame entry point
// Ported from NihAV binkvid.rs lines 475-950, 1146-1250
// ---------------------------------------------------------------------------

fn calc_len(size: usize) -> u8 {
    (32 - ((size + 511) as u32).leading_zeros()) as u8
}

#[allow(dead_code)]
pub struct BinkVideoDecoder {
    is_ver_b: bool,
    is_ver_i: bool,
    has_alpha: bool,
    is_gray: bool,
    swap_uv: bool,
    key_frame: bool,

    width: usize,
    height: usize,
    orig_width: usize,
    orig_height: usize,
    cur_w: usize,
    cur_h: usize,
    cur_plane: usize,

    colhi_tree: [Tree; 16],
    col_last: u8,

    btype: Bundle<u8>,
    sbtype: Bundle<u8>,
    colors: Bundle<u8>,
    pattern: Bundle<u8>,
    xoff: Bundle<i8>,
    yoff: Bundle<i8>,
    intradc: Bundle<u16>,
    interdc: Bundle<i16>,
    intraq: Bundle<u8>,
    interq: Bundle<u8>,
    nresidues: Bundle<u8>,
    run: Bundle<u8>,

    trees: BinkTrees,
    qmat_b: QuantMats,

    prev_y: Option<Vec<u8>>,
    prev_u: Option<Vec<u8>>,
    prev_v: Option<Vec<u8>>,
}

impl BinkVideoDecoder {
    pub fn new(version_char: u8, flags: u32, width: usize, height: usize) -> Self {
        let is_ver_b = version_char == b'b';
        let is_ver_i = version_char >= b'i';
        let has_alpha = (flags & BINK_FLAG_ALPHA) != 0;
        let is_gray = (flags & BINK_FLAG_GRAY) != 0;
        let swap_uv = version_char >= b'h';

        let w = (width + 7) & !7;
        let h = (height + 7) & !7;
        let bw = (width + 7) >> 3;
        let bh = (height + 7) >> 3;

        let mut dec = Self {
            is_ver_b,
            is_ver_i,
            has_alpha,
            is_gray,
            swap_uv,
            key_frame: true,
            width: w,
            height: h,
            orig_width: width,
            orig_height: height,
            cur_w: w,
            cur_h: h,
            cur_plane: 0,
            colhi_tree: Default::default(),
            col_last: 0,
            btype: Default::default(),
            sbtype: Default::default(),
            colors: Default::default(),
            pattern: Default::default(),
            xoff: Default::default(),
            yoff: Default::default(),
            intradc: Default::default(),
            interdc: Default::default(),
            intraq: Default::default(),
            interq: Default::default(),
            nresidues: Default::default(),
            run: Default::default(),
            trees: BinkTrees::default(),
            qmat_b: QuantMats::default(),
            prev_y: None,
            prev_u: None,
            prev_v: None,
        };

        dec.init_bundle_bufs(bw, bh);
        if is_ver_b {
            dec.qmat_b.calc_binkb_quants();
        }
        dec
    }

    fn init_bundle_bufs(&mut self, bw: usize, bh: usize) {
        let size = bw * bh * 64;
        self.btype.data.resize(size, 0);
        self.sbtype.data.resize(size, 0);
        self.colors.data.resize(size, 0);
        self.pattern.data.resize(size, 0);
        self.xoff.data.resize(size, 0);
        self.yoff.data.resize(size, 0);
        self.intradc.data.resize(size, 0);
        self.interdc.data.resize(size, 0);
        self.intraq.data.resize(size, 0);
        self.interq.data.resize(size, 0);
        self.nresidues.data.resize(size, 0);
        self.run.data.resize(size, 0);
    }

    fn init_bundle_lengths(&mut self, w: usize, bw: usize) {
        let w = (w + 7) & !7;
        self.btype.bits = calc_len(w >> 3);
        self.sbtype.bits = calc_len(w >> 4);
        self.colors.bits = calc_len(bw * 64);
        self.pattern.bits = calc_len(bw * 8);
        self.xoff.bits = calc_len(w >> 3);
        self.yoff.bits = calc_len(w >> 3);
        self.intradc.bits = calc_len(w >> 3);
        self.interdc.bits = calc_len(w >> 3);
        self.run.bits = calc_len(bw * 48);
    }

    fn init_bundle_lengths_binkb(&mut self) {
        self.btype.binkb_reset(4);
        self.colors.binkb_reset(8);
        self.pattern.binkb_reset(8);
        self.xoff.binkb_reset(5);
        self.yoff.binkb_reset(5);
        self.intradc.binkb_reset(11);
        self.interdc.binkb_reset(11);
        self.intraq.binkb_reset(4);
        self.interq.binkb_reset(4);
        self.nresidues.binkb_reset(7);
    }

    fn read_bundles_desc(&mut self, br: &mut BitReader) -> Result<(), String> {
        self.btype.read_desc(br)?;
        self.sbtype.read_desc(br)?;
        for el in &mut self.colhi_tree {
            el.read_desc(br)?;
        }
        self.col_last = 0;
        self.colors.read_desc(br)?;
        self.pattern.read_desc(br)?;
        self.xoff.read_desc(br)?;
        self.yoff.read_desc(br)?;
        self.intradc.reset();
        self.interdc.reset();
        self.run.read_desc(br)?;
        Ok(())
    }

    fn read_bundles_binkb(&mut self, br: &mut BitReader) -> Result<(), String> {
        self.btype.read_binkb(br)?;
        self.colors.read_binkb(br)?;
        self.pattern.read_binkb(br)?;
        self.xoff.read_binkb(br)?;
        self.yoff.read_binkb(br)?;
        self.intradc.read_binkb(br)?;
        self.interdc.read_binkb(br)?;
        self.intraq.read_binkb(br)?;
        self.interq.read_binkb(br)?;
        self.nresidues.read_binkb(br)?;
        Ok(())
    }

    fn read_bundles(&mut self, br: &mut BitReader) -> Result<(), String> {
        self.btype.read_block_types(br, &self.trees)?;
        self.sbtype.read_block_types(br, &self.trees)?;
        self.colors.read_colors(
            br,
            &self.trees,
            &self.colhi_tree,
            &mut self.col_last,
            self.is_ver_i,
        )?;
        self.pattern.read_patterns(br, &self.trees)?;
        self.xoff.read_motion_values(br, &self.trees)?;
        self.yoff.read_motion_values(br, &self.trees)?;
        self.intradc.read_dcs(br, DC_START_BITS)?;
        self.interdc.read_dcs(br, DC_START_BITS)?;
        self.run.read_runs(br, &self.trees)?;
        Ok(())
    }

    fn copy_block(
        &self,
        dst: &mut [u8],
        mut off: usize,
        stride: usize,
        bx: usize,
        by: usize,
        xoff: i8,
        yoff: i8,
    ) -> Result<(), String> {
        let prev = match self.cur_plane {
            0 => self.prev_y.as_ref(),
            1 => self.prev_u.as_ref(),
            2 => self.prev_v.as_ref(),
            _ => None,
        }
        .ok_or("bink: missing reference frame".to_string())?;
        let xoff_i = (bx * 8) as isize + xoff as isize;
        let yoff_i = (by * 8) as isize + yoff as isize;
        if xoff_i < 0
            || xoff_i + 8 > self.cur_w as isize
            || yoff_i < 0
            || yoff_i + 8 > self.cur_h as isize
        {
            return Err("bink: copy_block out of bounds".to_string());
        }
        let pstride = self.cur_w;
        let mut poff = xoff_i as usize + yoff_i as usize * pstride;
        for _ in 0..8 {
            dst[off..off + 8].copy_from_slice(&prev[poff..poff + 8]);
            off += stride;
            poff += pstride;
        }
        Ok(())
    }

    fn copy_overlapped(
        &self,
        dst: &mut [u8],
        mut off: usize,
        stride: usize,
        bx: usize,
        by: usize,
        xoff: i8,
        yoff1: i8,
    ) -> Result<(), String> {
        let ybias = if self.key_frame { -15 } else { 0 };
        let yoff = yoff1 as isize + ybias as isize;

        let xpos = (bx * 8) as isize + xoff as isize;
        let ypos = (by * 8) as isize + yoff;
        if xpos < 0 || xpos + 8 > self.cur_w as isize || ypos < 0 || ypos + 8 > self.cur_h as isize
        {
            return Err("bink: copy_overlapped out of bounds".to_string());
        }

        let mut block: [u8; 64] = [0; 64];
        let mut ref_off =
            (off as isize + xoff as isize + (yoff1 as isize + ybias as isize) * stride as isize)
                as usize;
        for row in block.chunks_exact_mut(8) {
            row.copy_from_slice(&dst[ref_off..ref_off + 8]);
            ref_off += stride;
        }
        for row in block.chunks_exact(8) {
            dst[off..off + 8].copy_from_slice(row);
            off += stride;
        }

        Ok(())
    }

    fn handle_block(
        &mut self,
        br: &mut BitReader,
        bx: usize,
        by: usize,
        dst: &mut [u8],
        off: usize,
        stride: usize,
        btype: u8,
        scaled: bool,
    ) -> Result<(), String> {
        let mut oblock: [u8; 64] = [0; 64];
        let mut coeffs: [i32; 64] = [0; 64];
        match btype {
            SKIP_BLOCK => {
                if scaled {
                    return Err("bink: skip block in scaled context".to_string());
                }
                self.copy_block(dst, off, stride, bx, by, 0, 0)?;
            }
            SCALED_BLOCK => {
                if scaled {
                    return Err("bink: nested scaled block".to_string());
                }
                let sbtype = self.sbtype.get_val()?;
                self.handle_block(br, bx, by, dst, off, stride, sbtype, true)?;
            }
            MOTION_BLOCK => {
                if scaled {
                    return Err("bink: motion block in scaled context".to_string());
                }
                let xoff = self.xoff.get_val()?;
                let yoff = self.yoff.get_val()?;
                self.copy_block(dst, off, stride, bx, by, xoff, yoff)?;
            }
            RUN_BLOCK => {
                let scan = BINK_PATTERNS[br.read(4)? as usize];
                let mut idx = 0;
                while idx < 63 {
                    let run = (self.run.get_val()? as usize) + 1;
                    if idx + run > 64 {
                        return Err("bink: run block past 64".to_string());
                    }
                    if br.read_bool()? {
                        let val = self.colors.get_val()?;
                        for j in 0..run {
                            oblock[scan[idx + j] as usize] = val;
                        }
                        idx += run;
                    } else {
                        for _ in 0..run {
                            oblock[scan[idx] as usize] = self.colors.get_val()?;
                            idx += 1;
                        }
                    }
                }
                if idx == 63 {
                    oblock[scan[63] as usize] = self.colors.get_val()?;
                }
                put_block(&oblock, dst, off, stride, scaled);
            }
            RESIDUE_BLOCK => {
                if scaled {
                    return Err("bink: residue block in scaled context".to_string());
                }
                let xoff = self.xoff.get_val()?;
                let yoff = self.yoff.get_val()?;
                self.copy_block(dst, off, stride, bx, by, xoff, yoff)?;
                let nmasks = br.read(7)? as usize;
                read_residue(br, &mut coeffs, nmasks)?;
                add_block(&coeffs, dst, off, stride);
            }
            INTRA_BLOCK => {
                coeffs[0] = self.intradc.get_val()? as i32;
                read_dct_coefficients(br, &mut coeffs, &BINK_SCAN, &BINK_INTRA_QUANT, None)?;
                if !scaled {
                    idct_put(&coeffs, dst, off, stride);
                } else {
                    idct_put(&coeffs, &mut oblock, 0, 8);
                    put_block(&oblock, dst, off, stride, scaled);
                }
            }
            FILL_BLOCK => {
                let fill = self.colors.get_val()?;
                oblock = [fill; 64];
                put_block(&oblock, dst, off, stride, scaled);
            }
            INTER_BLOCK => {
                if scaled {
                    return Err("bink: inter block in scaled context".to_string());
                }
                let xoff = self.xoff.get_val()?;
                let yoff = self.yoff.get_val()?;
                self.copy_block(dst, off, stride, bx, by, xoff, yoff)?;
                coeffs[0] = self.interdc.get_val()? as i32;
                read_dct_coefficients(br, &mut coeffs, &BINK_SCAN, &BINK_INTER_QUANT, None)?;
                idct_add(&coeffs, dst, off, stride);
            }
            PATTERN_BLOCK => {
                let clr: [u8; 2] = [self.colors.get_val()?, self.colors.get_val()?];
                for i in 0..8 {
                    let pattern = self.pattern.get_val()? as usize;
                    for j in 0..8 {
                        oblock[i * 8 + j] = clr[(pattern >> j) & 1];
                    }
                }
                put_block(&oblock, dst, off, stride, scaled);
            }
            RAW_BLOCK => {
                for i in 0..8 {
                    for j in 0..8 {
                        oblock[i * 8 + j] = self.colors.get_val()?;
                    }
                }
                put_block(&oblock, dst, off, stride, scaled);
            }
            _ => {
                return Err(format!("bink: unknown block type {btype}"));
            }
        };
        Ok(())
    }

    fn decode_plane(
        &mut self,
        br: &mut BitReader,
        dst: &mut [u8],
        stride: usize,
        plane_width: usize,
        plane_height: usize,
    ) -> Result<(), String> {
        let bw = (plane_width + 7) >> 3;
        let bh = (plane_height + 7) >> 3;
        self.cur_w = (plane_width + 7) & !7;
        self.cur_h = (plane_height + 7) & !7;
        self.init_bundle_lengths(plane_width.max(8), bw);
        self.read_bundles_desc(br)?;
        let mut off = 0;
        for _by in 0..bh {
            self.read_bundles(br)?;
            let mut bx = 0;
            while bx < bw {
                let btype = self.btype.get_val()?;
                if btype == SCALED_BLOCK && (_by & 1) == 1 {
                    bx += 2;
                    continue;
                }
                self.handle_block(br, bx, _by, dst, off + bx * 8, stride, btype, false)?;
                if btype == SCALED_BLOCK {
                    bx += 1;
                }
                bx += 1;
            }
            off += stride * 8;
        }
        if (br.tell() & 0x1F) != 0 {
            let skip = 32 - (br.tell() & 0x1F);
            br.skip(skip)?;
        }
        Ok(())
    }

    fn decode_plane_binkb(
        &mut self,
        br: &mut BitReader,
        dst: &mut [u8],
        stride: usize,
        plane_width: usize,
        plane_height: usize,
    ) -> Result<(), String> {
        let bw = (plane_width + 7) >> 3;
        let bh = (plane_height + 7) >> 3;
        self.cur_w = (plane_width + 7) & !7;
        self.cur_h = (plane_height + 7) & !7;
        self.cur_plane = 0; // reset for each plane call
        self.init_bundle_lengths_binkb();
        let mut off = 0;
        for _by in 0..bh {
            self.read_bundles_binkb(br)?;
            for bx in 0..bw {
                let mut coeffs: [i32; 64] = [0; 64];
                let btype = self.btype.get_val()?;
                match btype {
                    0 => { /* skip */ }
                    1 => {
                        // run (binkb)
                        let scan = BINK_PATTERNS[br.read(4)? as usize];
                        let mut idx = 0;
                        while idx < 63 {
                            let run = br.read_bool()?;
                            let len = (br.read(BINKB_RUN_BITS[idx])? as usize) + 1;
                            if idx + len > 64 {
                                return Err("bink: binkb run past 64".to_string());
                            }
                            if run {
                                let val = self.colors.get_val()?;
                                for j in 0..len {
                                    let pos = scan[idx + j] as usize;
                                    dst[off + (pos >> 3) * stride + (pos & 7)] = val;
                                }
                                idx += len;
                            } else {
                                for _ in 0..len {
                                    let pos = scan[idx] as usize;
                                    dst[off + (pos >> 3) * stride + (pos & 7)] =
                                        self.colors.get_val()?;
                                    idx += 1;
                                }
                            }
                        }
                        if idx == 63 {
                            let pos = scan[idx] as usize;
                            dst[off + (pos >> 3) * stride + (pos & 7)] = self.colors.get_val()?;
                        }
                    }
                    2 => {
                        // intra (binkb)
                        coeffs[0] = self.intradc.get_val()? as i32;
                        let q = self.intraq.get_val()? as usize;
                        read_dct_coefficients(
                            br,
                            &mut coeffs,
                            &BINK_SCAN,
                            &self.qmat_b.intra_qmat,
                            Some(q),
                        )?;
                        idct_put(&coeffs, dst, off, stride);
                    }
                    3 => {
                        // residue (binkb)
                        let xoff = self.xoff.get_val()?;
                        let yoff = self.yoff.get_val()?;
                        self.copy_overlapped(dst, off, stride, bx, _by, xoff, yoff)?;
                        let nmasks = self.nresidues.get_val()? as usize;
                        read_residue(br, &mut coeffs, nmasks)?;
                        add_block(&coeffs, dst, off, stride);
                    }
                    4 => {
                        // inter (binkb)
                        let xoff = self.xoff.get_val()?;
                        let yoff = self.yoff.get_val()?;
                        self.copy_overlapped(dst, off, stride, bx, _by, xoff, yoff)?;
                        coeffs[0] = self.interdc.get_val()? as i32;
                        let q = self.interq.get_val()? as usize;
                        read_dct_coefficients(
                            br,
                            &mut coeffs,
                            &BINK_SCAN,
                            &self.qmat_b.inter_qmat,
                            Some(q),
                        )?;
                        idct_add(&coeffs, dst, off, stride);
                    }
                    5 => {
                        // fill (binkb)
                        let fill = self.colors.get_val()?;
                        for i in 0..8 {
                            for j in 0..8 {
                                dst[off + i * stride + j] = fill;
                            }
                        }
                    }
                    6 => {
                        // pattern (binkb)
                        let clr: [u8; 2] = [self.colors.get_val()?, self.colors.get_val()?];
                        for i in 0..8 {
                            let pattern = self.pattern.get_val()? as usize;
                            for j in 0..8 {
                                dst[off + i * stride + j] = clr[(pattern >> j) & 1];
                            }
                        }
                    }
                    7 => {
                        // motion (binkb)
                        let xoff = self.xoff.get_val()?;
                        let yoff = self.yoff.get_val()?;
                        self.copy_overlapped(dst, off, stride, bx, _by, xoff, yoff)?;
                    }
                    8 => {
                        // raw (binkb)
                        for i in 0..8 {
                            for j in 0..8 {
                                dst[off + i * stride + j] = self.colors.get_val()?;
                            }
                        }
                    }
                    _ => {
                        return Err(format!("bink: unknown binkb block type {btype}"));
                    }
                };
                off += 8;
            }
            off += stride * 8 - bw * 8;
        }
        if (br.tell() & 0x1F) != 0 {
            let skip = 32 - (br.tell() & 0x1F);
            br.skip(skip)?;
        }
        Ok(())
    }

    pub fn decode_frame(&mut self, packet: &[u8]) -> Result<Vec<u8>, String> {
        let mut br = BitReader::new(packet);

        let w = self.width;
        let h = self.height;
        let cw = w / 2;
        let ch = h / 2;

        self.key_frame = self.prev_y.is_none();

        let mut y_plane = vec![0u8; w * h];
        let mut u_plane = vec![0u8; cw * ch];
        let mut v_plane = vec![0u8; cw * ch];

        if self.has_alpha
            && self.is_ver_i {
                br.skip(32)?;
            }
            // Skip alpha plane decoding — not critical for gameplay
        if self.is_ver_i {
            br.skip(32)?;
        }

        let nplanes = if self.is_gray { 1 } else { 3 };
        for plane in 0..nplanes {
            if self.is_ver_b {
                match plane {
                    0 => {
                        self.cur_plane = 0;
                        self.decode_plane_binkb(&mut br, &mut y_plane, w, w, h)?;
                    }
                    1 => {
                        self.cur_plane = 1;
                        self.decode_plane_binkb(&mut br, &mut u_plane, cw, cw, ch)?;
                    }
                    2 => {
                        self.cur_plane = 2;
                        self.decode_plane_binkb(&mut br, &mut v_plane, cw, cw, ch)?;
                    }
                    _ => {}
                }
            } else {
                let plane_idx = if plane > 0 && self.swap_uv {
                    plane ^ 3
                } else {
                    plane
                };
                match plane_idx {
                    0 => {
                        self.cur_plane = 0;
                        self.decode_plane(&mut br, &mut y_plane, w, w, h)?;
                    }
                    1 => {
                        self.cur_plane = 1;
                        self.decode_plane(&mut br, &mut u_plane, cw, cw, ch)?;
                    }
                    2 => {
                        self.cur_plane = 2;
                        self.decode_plane(&mut br, &mut v_plane, cw, cw, ch)?;
                    }
                    _ => {}
                }
            }
        }

        self.prev_y = Some(y_plane.clone());
        self.prev_u = Some(u_plane.clone());
        self.prev_v = Some(v_plane.clone());

        // Convert YUV (BT.601 full range) to RGBA
        let mut rgba = vec![0u8; self.orig_width * self.orig_height * 4];
        for py in 0..self.orig_height {
            for px in 0..self.orig_width {
                let y = y_plane[py * w + px] as f32;
                let u = u_plane[(py / 2) * cw + (px / 2)] as f32 - 128.0;
                let v = v_plane[(py / 2) * cw + (px / 2)] as f32 - 128.0;
                let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
                let g = (y - 0.344_136 * u - 0.714_136 * v).clamp(0.0, 255.0) as u8;
                let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;
                let dst = (py * self.orig_width + px) * 4;
                rgba[dst] = r;
                rgba[dst + 1] = g;
                rgba[dst + 2] = b;
                rgba[dst + 3] = 255;
            }
        }
        Ok(rgba)
    }
}
