//! WWLib Rust Implementation
//!
//! This crate provides Rust implementations of components from the WWLib library
//! used in Command & Conquer Generals and Zero Hour.

pub mod base_type;
pub mod blowfish;
pub mod r#bool;
pub mod borlandc;
pub mod crc;
pub mod fast_allocator;
pub mod global;
pub mod ini;
pub mod ini_parser;
pub mod lcw;
pub mod lzo;
pub mod md5;
pub mod memory_system;
pub mod noinit;
pub mod point;
pub mod random;
pub mod rawfile;
pub mod sha;
pub mod string_system;
pub mod surface;
pub mod vector_class;
pub mod wwfile;

pub mod _convert;
pub mod _mono;
pub mod _timer;
pub mod _xmouse;
pub mod always;
pub mod argv;
pub mod b64pipe;
pub mod b64straw;
pub mod base64;
pub mod bfiofile;
pub mod binheap;
pub mod bittype;
pub mod blit;
pub mod blitblit;
pub mod blitter;
pub mod blowpipe;
pub mod blwstraw;
pub mod bound;
pub mod bsearch;
pub mod bsurface;
pub mod buff;
pub mod bufffile;
pub mod callback_hook;
pub mod chunkio;
pub mod convert;
pub mod cpudetect;
pub mod crandom;
pub mod crcpipe;
pub mod crcstraw;
pub mod critsection;
pub mod cstraw;
pub mod data;
pub mod ddraw;
pub mod dib;
pub mod draw;
pub mod dsurface;
pub mod except;
pub mod ffactory;
pub mod fixed;
pub mod font;
pub mod gcd_lcm;
pub mod hash;
pub mod hashcalc;
pub mod hashlist;
pub mod hashtab;
pub mod hashtemplate;
pub mod hsv;
pub mod iff;
pub mod incdec;
pub mod index;
pub mod inisup;
pub mod int;
pub mod iostruct;
pub mod keyboard;
pub mod launch_web;
pub mod lcwpipe;
pub mod listnode;
pub mod load;
pub mod lzo1x;
pub mod lzo1x_c;
pub mod lzo1x_d;
pub mod lzo_conf;
pub mod lzoconf;
pub mod lzopipe;
pub mod lzostraw;
pub mod mempool;
pub mod misc;
pub mod mixfile;
pub mod mmsys;
pub mod mono;
pub mod monodrvr;
pub mod mpmath;
pub mod mpu;
pub mod msgloop;
pub mod multilist;
pub mod mutex;
pub mod notifier;
pub mod nstrdup;
pub mod ntree;
pub mod obscure;
pub mod palette;
pub mod pcx;
pub mod pipe;
pub mod pk;
pub mod pkpipe;
pub mod pkstraw;
pub mod ramfile;
pub mod rc4;
pub mod rcfile;
pub mod readline;
pub mod realcrc;
pub mod ref_ptr;
pub mod refcount;
pub mod regexpr;
pub mod registry;
pub mod rgb;
pub mod rle;
pub mod rlerle;
pub mod rndstraw;
pub mod rndstrng;
pub mod rng;
pub mod rsacrypt;
pub mod sampler;
pub mod search;
pub mod shapeset;
pub mod shapipe;
pub mod sharebuf;
pub mod shastraw;
pub mod signaler;
pub mod simplevec;
pub mod slist;
pub mod slnode;
pub mod smartptr;
pub mod srandom;
pub mod stimer;
pub mod stl;
pub mod straw;
pub mod strtok_r;
pub mod surfrect;
pub mod swap;
pub mod systimer;
pub mod tagblock;
pub mod targa;
pub mod textfile;
pub mod tgatodxt;
pub mod thread;
pub mod timer;
pub mod trackwin;
pub mod trackxy;
pub mod trect;
pub mod trig;
pub mod trim;
pub mod uarray;
pub mod vector;
pub mod verchk;
pub mod visualc;
pub mod watcom;
pub mod widestring;
pub mod win;
pub mod wwcomutil;
pub mod wwfont;
pub mod wwmouse;
pub mod xmouse;
pub mod xpipe;
pub mod xstraw;
pub mod xsurface;
pub use always::{
    allocate_from_w3d_mem_pool, allocate_from_w3d_mem_pool_with_msg, create_w3d_mem_pool,
    free_from_w3d_mem_pool, ww_max, ww_min, W3dMemPool, W3dMpo,
};
pub use b64pipe::{Base64Pipe, CodeControl};
pub use base64::{base64_decode, base64_encode};
pub use base_types::*;
pub use blit::{
    bit_blit, bit_blit_clipped, buffer_size, from_buffer, rle_blit, rle_blit_clipped, to_buffer,
};
pub use blitter::{
    BlitDarken, BlitPlainU16, BlitPlainU32, BlitPlainU8, BlitPlainXlat, BlitTransDarken,
    BlitTransLucent25, BlitTransLucent50, BlitTransLucent75, BlitTransRemapDest,
    BlitTransRemapXlat, BlitTransU16, BlitTransU32, BlitTransU8, BlitTransXlat,
    BlitTransZRemapXlat, Blitter, RLEBlitter,
};
pub use blowfish::{BlowfishEngine, BlowfishError, BlowfishResult, BLOCK_SIZE, MAX_KEY_LENGTH};
pub use blowpipe::{BlowPipe, CryptControl};
pub use callback_hook::{Callback, CallbackHook};
pub use chunkio::{ChunkHeader, ChunkLoadClass, ChunkSaveClass, MicroChunkHeader};
pub use convert::ConvertClass;
pub use crc::*;
pub use crcpipe::CrcPipe;
pub use crcstraw::CrcStraw;
pub use fast_allocator::*;
pub use font::FontClass;
pub use global::{POINTER, UINT2, UINT4};
pub use hashcalc::HashCalculator;
pub use hsv::*;
pub use ini::{INIClass, INIEntry, INIError, INIResult, INISection};
pub use iostruct::*;
pub use keyboard::{KeyboardClass, WWKeyboardClass};
pub use lcw::{compress, decompress, LcwError, LcwResult};
pub use lcwpipe::{CompControl, LcwPipe};
pub use lzo::{lzo_buffer_size, LzoCompressor, LzoError, LzoResult};
pub use lzopipe::LzoPipe;
pub use lzostraw::LzoStraw;
pub use md5::*;
pub use misc::{
    debug_windowed, delay, get_free_video_memory, get_video_hardware_capabilities,
    prep_direct_draw, process_dd_result, reset_video_mode, set_debug_windowed, set_palette,
    set_palette_raw, set_surfaces_restored, set_video_mode, surfaces_restored,
    trigger_audio_focus_loss, vsync, wait_blit, wait_vert_blank,
};
pub use msgloop::{
    add_accelerator, add_modeless_dialog, remove_accelerator, remove_modeless_dialog,
    windows_message_handler,
};
pub use noinit::NoInit;
pub use obscure::{obfuscate, obfuscate_bytes};
pub use palette::*;
pub use pipe::{put_to, Pipe, PipeBase};
pub use point::*;
pub use r#bool::{Bool, BoolInt, FALSE, TRUE};
pub use random::{
    pick_random_number, Random2Class, Random3Class, Random4Class, RandomClass, RandomGenerator,
};
pub use rawfile::*;
pub use realcrc::{crc_memory, crc_string, crc_stringi};
pub use ref_ptr::{RefCountPtr, RefCounted};
pub use registry::RegistryClass;
pub use rgb::*;
pub use rlerle::{
    RLEBlitTransDarken, RLEBlitTransLucent25, RLEBlitTransLucent50, RLEBlitTransLucent75,
    RLEBlitTransRemapDest, RLEBlitTransRemapXlat, RLEBlitTransXlat, RLEBlitTransZRemapXlat,
};
pub use sha::*;
pub use straw::{get_from, Straw, StrawBase};
pub use surface::*;
pub use trig::*;
pub use uarray::UniqueArrayClass;
pub use vector_class::*;
pub use verchk::*;
pub use win::{is_game_in_focus, print_win32_error, set_game_in_focus};
// Note: wwfile module is available but not glob-imported to avoid name conflicts with rawfile
pub use wwfile::{datetime, FileInterface, WWFile};
