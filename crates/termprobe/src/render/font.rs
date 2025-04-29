use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use super::style::FontFamilies;

pub(crate) const FAMILY: &str = "JetBrains Mono";

#[cfg(feature = "recording-font-jetbrains-mono")]
const REGULAR_FONTS: &[&[u8]] = &[include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/jetbrains-mono/JetBrainsMono-Regular.ttf"
))];

#[cfg(feature = "recording-font-jetbrains-mono-styles")]
const STYLED_FONTS: &[&[u8]] = &[
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-Bold.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-Italic.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-BoldItalic.ttf"
    )),
];

#[cfg(feature = "recording-font-jetbrains-mono-full")]
const FULL_FAMILY_FONTS: &[&[u8]] = &[
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-Thin.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-ThinItalic.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-ExtraLight.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-ExtraLightItalic.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-Light.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-LightItalic.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-Medium.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-MediumItalic.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-SemiBold.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-SemiBoldItalic.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-ExtraBold.ttf"
    )),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/jetbrains-mono/JetBrainsMono-ExtraBoldItalic.ttf"
    )),
];

pub(crate) struct Catalog {
    pub database: Arc<fontdb::Database>,
    candidates: [Vec<fontdb::ID>; 4],
    nerd_faces: Vec<fontdb::ID>,
}

impl Catalog {
    pub fn candidates(&self, bold: bool, italic: bool, character: char) -> Vec<fontdb::ID> {
        let mut output = Vec::new();
        let mut seen = HashSet::new();
        if super::nerd_font::is_private_use(character) {
            output.extend(
                self.nerd_faces
                    .iter()
                    .copied()
                    .filter(|id| seen.insert(*id)),
            );
        }
        output.extend(
            self.candidates[style_index(bold, italic)]
                .iter()
                .copied()
                .filter(|id| seen.insert(*id)),
        );
        output
    }
}

/// Scan system fonts once; each style adds its files to a clone.
fn base_database() -> &'static fontdb::Database {
    static BASE: OnceLock<fontdb::Database> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut database = fontdb::Database::new();
        load_bundled_fonts(&mut database);
        database.load_font_data(super::nerd_font::FONT_DATA.to_vec());
        database.load_system_fonts();
        database
    })
}

static CATALOGS: OnceLock<Mutex<HashMap<FontFamilies, Weak<Catalog>>>> = OnceLock::new();

pub(crate) fn catalog_for(font: &FontFamilies) -> anyhow::Result<Arc<Catalog>> {
    let mut catalogs = CATALOGS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    catalogs.retain(|_, catalog| catalog.strong_count() > 0);
    if let Some(existing) = catalogs.get(font).and_then(Weak::upgrade) {
        return Ok(existing);
    }
    // Hold the lock to avoid duplicate, expensive font ranking.
    let catalog = Arc::new(build_catalog(font)?);
    catalogs.insert(font.clone(), Arc::downgrade(&catalog));
    Ok(catalog)
}

/// Bound total custom font data from untrusted config files.
const MAX_FONT_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Unlike fontdb's path loader, read only regular files with a size limit.
/// Unavailable files use the normal font fallback.
fn load_named_font(
    database: &mut fontdb::Database,
    path: &Path,
    remaining: &mut u64,
) -> anyhow::Result<()> {
    if !path.metadata().is_ok_and(|meta| meta.is_file()) {
        return Ok(());
    }
    let Ok(file) = File::open(path) else {
        return Ok(());
    };
    let Ok(metadata) = file.metadata() else {
        return Ok(());
    };
    if !metadata.is_file() {
        return Ok(());
    }
    if metadata.len() > *remaining {
        anyhow::bail!("custom font data exceeds the {MAX_FONT_FILE_BYTES} byte limit");
    }
    // Read one extra byte to detect growth after the metadata check.
    let mut data = Vec::new();
    let read = file.take(*remaining + 1).read_to_end(&mut data);
    if data.len() as u64 > *remaining {
        anyhow::bail!("custom font data exceeds the {MAX_FONT_FILE_BYTES} byte limit");
    }
    if read.is_ok() {
        *remaining -= data.len() as u64;
        database.load_font_data(data);
    }
    Ok(())
}

fn build_catalog(font: &FontFamilies) -> anyhow::Result<Catalog> {
    let mut database = base_database().clone();
    let mut seen = HashSet::new();
    let mut remaining = MAX_FONT_FILE_BYTES;
    for path in &font.files {
        if let Ok(path) = path.canonicalize() {
            if seen.insert(path.clone()) {
                load_named_font(&mut database, &path, &mut remaining)?;
            }
        }
    }

    let candidates = std::array::from_fn(|index| {
        let bold = index & 1 != 0;
        let italic = index & 2 != 0;
        let preferred = preferred_families(font, bold, italic);
        let mut faces = database.faces().collect::<Vec<_>>();
        faces.sort_by(|left, right| {
            face_score(left, &preferred, bold, italic)
                .partial_cmp(&face_score(right, &preferred, bold, italic))
                .unwrap_or(Ordering::Equal)
        });
        faces.into_iter().map(|face| face.id).collect()
    });
    let nerd_faces = database
        .faces()
        .filter(|face| {
            face.families
                .iter()
                .any(|(family, _)| family.contains("Nerd Font"))
        })
        .map(|face| face.id)
        .collect();
    Ok(Catalog {
        database: Arc::new(database),
        candidates,
        nerd_faces,
    })
}

fn load_bundled_fonts(database: &mut fontdb::Database) {
    #[cfg(feature = "recording-font-jetbrains-mono")]
    load_font_data(database, REGULAR_FONTS);
    #[cfg(feature = "recording-font-jetbrains-mono-styles")]
    load_font_data(database, STYLED_FONTS);
    #[cfg(feature = "recording-font-jetbrains-mono-full")]
    load_font_data(database, FULL_FAMILY_FONTS);
    #[cfg(not(feature = "recording-font-jetbrains-mono"))]
    let _ = database;
}

#[cfg(feature = "recording-font-jetbrains-mono")]
fn load_font_data(database: &mut fontdb::Database, fonts: &[&[u8]]) {
    for font in fonts {
        database.load_font_data(font.to_vec());
    }
}

fn style_index(bold: bool, italic: bool) -> usize {
    usize::from(bold) | (usize::from(italic) << 1)
}
