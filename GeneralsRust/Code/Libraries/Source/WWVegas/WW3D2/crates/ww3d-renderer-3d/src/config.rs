use std::env;
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilterQuality {
    Bilinear,
    Trilinear,
    Anisotropic,
}

#[derive(Debug, Clone, Copy)]
pub struct RendererConfig {
    pub prefer_16bit_textures: bool,
    pub force_srgb_textures: bool,
    pub filter_quality: TextureFilterQuality,
    pub max_anisotropy: u16,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            prefer_16bit_textures: false,
            force_srgb_textures: false,
            filter_quality: TextureFilterQuality::Anisotropic,
            max_anisotropy: 8,
        }
    }
}

static CONFIG: OnceLock<RwLock<RendererConfig>> = OnceLock::new();

pub fn init_from_env() {
    let _ = CONFIG.set(RwLock::new(RendererConfig::default()));
    // Update from env
    let prefer_16 =
        env::var("WW3D_PREFER_16BIT_TEXTURES").map(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let srgb =
        env::var("WW3D_FORCE_SRGB_TEXTURES").map(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let filter = env::var("WW3D_TEXTURE_FILTER").ok();
    let max_aniso = env::var("WW3D_MAX_ANISOTROPY")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|&v| v >= 1);

    if let Some(lock) = CONFIG.get() {
        let mut cfg = lock.write().unwrap();
        if let Ok(v) = prefer_16 {
            cfg.prefer_16bit_textures = v;
        }
        if let Ok(v) = srgb {
            cfg.force_srgb_textures = v;
        }
        if let Some(value) = filter {
            cfg.filter_quality = match value.to_ascii_lowercase().as_str() {
                "bilinear" => TextureFilterQuality::Bilinear,
                "trilinear" => TextureFilterQuality::Trilinear,
                "anisotropic" => TextureFilterQuality::Anisotropic,
                _ => cfg.filter_quality,
            };
        }
        if let Some(aniso) = max_aniso {
            cfg.max_anisotropy = aniso.clamp(1, 16);
        }
    }
}

pub fn get() -> RendererConfig {
    CONFIG
        .get_or_init(|| RwLock::new(RendererConfig::default()))
        .read()
        .unwrap()
        .to_owned()
}

pub fn set(cfg: RendererConfig) {
    let lock = CONFIG.get_or_init(|| RwLock::new(RendererConfig::default()));
    let mut cfg = cfg;
    cfg.max_anisotropy = cfg.max_anisotropy.clamp(1, 16);
    *lock.write().unwrap() = cfg;
}
