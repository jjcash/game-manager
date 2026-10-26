// SPDX-License-Identifier: GPL-3.0-or-later
use image::{ImageFormat, Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const UNIQUE_ASSETS: u32 = 1024;
// The artwork fixture format validates independently of the feature-gated SQL generator.
const SUPPORTED_SIZES: [u32; 6] = [500, 2_000, 5_000, 12_320, 15_000, 25_000];
pub fn asset_for_index(index: u32) -> u32 {
    if index.is_multiple_of(5) {
        index % 32
    } else {
        index % UNIQUE_ASSETS
    }
}
const MARKER: &str = "synthetic-artwork-v1.json";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub games: u32,
    pub seed: u64,
}
pub fn read_config(root: &Path) -> Result<Config, String> {
    let path = root.join(MARKER);
    let bytes = super::read_owned(&path, 512).map_err(|_| "Invalid fixture marker")?;
    let config: Config = serde_json::from_slice(&bytes).map_err(|_| "Invalid fixture marker")?;
    if config.version != 1 || !SUPPORTED_SIZES.contains(&config.games) {
        return Err("Unsupported fixture dataset".into());
    }
    Ok(config)
}
fn validate_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute()
        || !root
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("artwork-"))
        || root.parent().and_then(Path::file_name) != Some(std::ffi::OsStr::new("data"))
        || root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            != Some(std::ffi::OsStr::new("benchmarks"))
    {
        return Err("Use an absolute benchmarks/data/artwork-NAME directory".into());
    }
    if root
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Parent traversal refused".into());
    }
    Ok(())
}
/// Versioned procedural RGB patterns, JPEG qualities and PNGs. Never downloads content.
pub fn generate(root: &Path, config: Config) -> Result<(), String> {
    validate_root(root)?;
    if config.version != 1 || !SUPPORTED_SIZES.contains(&config.games) {
        return Err("Unsupported size".into());
    }
    fs::create_dir_all(root.parent().ok_or("Missing parent")?).map_err(|e| e.to_string())?;
    fs::create_dir(root).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    for name in ["originals", "thumbnails", "temporary"] {
        fs::create_dir(root.join(name)).map_err(|e| e.to_string())?;
    }
    for asset in 0..config.games.min(UNIQUE_ASSETS) {
        let path = root.join("originals").join(format!("{asset:04}.img"));
        if asset.is_multiple_of(17) {
            continue;
        }
        if asset.is_multiple_of(19) {
            fs::write(path, b"\x89PNG\r\n\x1a\ncorrupt").map_err(|e| e.to_string())?;
            continue;
        }
        if asset.is_multiple_of(37) {
            fs::write(path, b"GIF89a unsupported synthetic fixture").map_err(|e| e.to_string())?;
            continue;
        }
        let (width, height) = if asset.is_multiple_of(31) {
            (4097, 1024)
        } else if asset.is_multiple_of(29) {
            (2048, 1024)
        } else {
            [(320, 180), (640, 360), (960, 540)][asset as usize % 3]
        };
        let image = pixels(config.seed, asset, width, height);
        let file = fs::File::create(path).map_err(|e| e.to_string())?;
        if asset.is_multiple_of(5) {
            image
                .write_to(&mut std::io::BufWriter::new(file), ImageFormat::Png)
                .map_err(|e| e.to_string())?;
        } else {
            image::codecs::jpeg::JpegEncoder::new_with_quality(
                file,
                [45, 70, 90][asset as usize % 3],
            )
            .encode_image(&image)
            .map_err(|e| e.to_string())?;
        }
    }
    // The marker denotes complete generation; incomplete directories are preserved.
    fs::write(
        root.join(MARKER),
        serde_json::to_vec(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn pixels(seed: u64, asset: u32, width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        let n = super::fingerprint(
            &(seed ^ u64::from(asset) ^ (u64::from(x / 4) << 32) ^ u64::from(y / 4)).to_le_bytes(),
        );
        let noise = if asset.is_multiple_of(3) {
            n as u8 / 2
        } else {
            0
        };
        Rgb([
            ((x * 255 / width) as u8).wrapping_add(noise),
            ((y * 255 / height) as u8).wrapping_add(asset as u8),
            (n >> 40) as u8 / 4 + 48,
        ])
    })
}
/// Explicit CLI cleanup only, constrained to a canonical marked benchmark root.
pub fn clean(root: &Path) -> Result<(), String> {
    validate_root(root)?;
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    validate_root(&canonical)?;
    read_config(&canonical)?;
    // Refuse links/reparse redirects or unrecognized content before recursive removal.
    fn inspect(path: &Path, depth: u32) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            let meta = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() || p.canonicalize().map_err(|e| e.to_string())? != p {
                return Err("Linked content preserved".into());
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if depth == 0 {
                if name == MARKER && meta.is_file() {
                    continue;
                }
                if !["originals", "thumbnails", "temporary"].contains(&name.as_str())
                    || !meta.is_dir()
                {
                    return Err("Unrecognized content preserved".into());
                }
                inspect(&p, 1)?;
            } else if !meta.is_file()
                || !(name.ends_with(".img") || name.ends_with(".png") || name.ends_with(".tmp"))
            {
                return Err("Unrecognized fixture content preserved".into());
            }
        }
        Ok(())
    }
    inspect(&canonical, 0)?;
    fs::remove_dir_all(canonical).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_pixels_encoding_and_seed_variation() {
        assert_eq!(SUPPORTED_SIZES, crate::storage::synthetic::DATASET_SIZES);
        let encode = |seed| {
            let mut bytes = std::io::Cursor::new(Vec::new());
            pixels(seed, 3, 64, 36)
                .write_to(&mut bytes, ImageFormat::Png)
                .unwrap();
            bytes.into_inner()
        };
        assert_eq!(encode(42), encode(42));
        assert_ne!(encode(42), encode(43));
        assert!(validate_root(Path::new("C:/personal/artwork-x")).is_err());
    }
    #[test]
    fn cleanup_refuses_unmarked_or_unrecognized_roots() {
        let temp = tempfile::tempdir().unwrap();
        assert!(clean(temp.path()).is_err());
    }
}
