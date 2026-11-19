//! Music-folder scanning: tags via `lofty`, cover images found next to the audio files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lofty::prelude::*;
use lofty::tag::ItemKey;
use serde::Serialize;
use walkdir::WalkDir;

const AUDIO_EXT: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "oga", "opus", "m4a", "mp4", "aac", "aif", "aiff", "aifc",
];
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif"];
const COVER_NAMES: &[&str] = &["cover", "folder", "front", "album", "artwork"];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub path: String,
    pub file_name: String,
    /// Folder names between the music folder and the file, used when tags are missing.
    pub rel_dirs: Vec<String>,
    /// Folder that contains the file (used to look up the album cover).
    pub dir: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<u32>,
    /// Seconds; 0 when unknown.
    pub duration: f64,
    pub has_picture: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub root: String,
    pub root_name: String,
    pub tracks: Vec<Track>,
    /// Folder → chosen image file in that folder.
    pub folder_covers: HashMap<String, String>,
}

fn ext_lower(p: &Path) -> Option<String> {
    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase())
}

fn is_cover_name(p: &Path) -> bool {
    p.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| {
            let s = s.to_ascii_lowercase();
            COVER_NAMES.iter().any(|n| s.starts_with(n))
        })
        .unwrap_or(false)
}

/// Any image in an album folder is taken as its cover. With several images, a conventional name
/// (cover / folder / front / album / artwork) wins; otherwise the largest file (usually the front scan).
fn better_cover(current: Option<&(PathBuf, u64)>, cand: &Path, size: u64) -> bool {
    match current {
        None => true,
        Some((cur, cur_size)) => {
            let (a, b) = (is_cover_name(cur), is_cover_name(cand));
            if a != b {
                b
            } else {
                size > *cur_size
            }
        }
    }
}

fn read_track(path: &Path, root: &Path) -> Track {
    let rel_dirs = path
        .parent()
        .and_then(|d| d.strip_prefix(root).ok())
        .map(|r| {
            r.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let mut t = Track {
        path: path.to_string_lossy().into_owned(),
        file_name: path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
        rel_dirs,
        dir: path
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default(),
        title: None,
        artist: None,
        album_artist: None,
        album: None,
        track: None,
        disc: None,
        year: None,
        duration: 0.0,
        has_picture: false,
    };
    if let Ok(tagged) = lofty::read_from_path(path) {
        t.duration = tagged.properties().duration().as_secs_f64();
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            let clean = |s: Option<std::borrow::Cow<'_, str>>| {
                s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
            };
            t.title = clean(tag.title());
            t.artist = clean(tag.artist());
            t.album = clean(tag.album());
            t.album_artist = tag
                .get_string(&ItemKey::AlbumArtist)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            t.track = tag.track();
            t.disc = tag.disk();
            t.year = tag.year();
            t.has_picture = !tag.pictures().is_empty();
        }
    }
    t
}

pub fn scan(root: &Path) -> Library {
    let mut tracks = Vec::new();
    let mut covers: HashMap<PathBuf, (PathBuf, u64)> = HashMap::new();

    for entry in WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            // skip hidden files/folders (".DS_Store", "._file" AppleDouble files, ".Trash" …)
            e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.')
        })
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(ext) = ext_lower(path) else { continue };
        if AUDIO_EXT.contains(&ext.as_str()) {
            tracks.push(read_track(path, root));
        } else if IMAGE_EXT.contains(&ext.as_str()) {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if let Some(dir) = path.parent() {
                if better_cover(covers.get(dir), path, size) {
                    covers.insert(dir.to_path_buf(), (path.to_path_buf(), size));
                }
            }
        }
    }

    Library {
        root: root.to_string_lossy().into_owned(),
        root_name: root
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        tracks,
        folder_covers: covers
            .into_iter()
            .map(|(d, (p, _))| (d.to_string_lossy().into_owned(), p.to_string_lossy().into_owned()))
            .collect(),
    }
}

/// Bytes of the first picture stored in the file's tags (front cover preferred).
pub fn embedded_picture(path: &Path) -> Result<Vec<u8>, String> {
    let tagged = lofty::read_from_path(path).map_err(|e| e.to_string())?;
    for tag in tagged.tags() {
        let pics = tag.pictures();
        let front = pics
            .iter()
            .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
            .or_else(|| pics.first());
        if let Some(p) = front {
            return Ok(p.data().to_vec());
        }
    }
    Err("no embedded picture".into())
}

#[cfg(test)]
mod tests {
    //! Run with `MUSICA_TEST_DIR=/path/to/folder cargo test -- --nocapture` against a real folder.
    use super::*;

    #[test]
    fn scan_folder_from_env() {
        let Ok(dir) = std::env::var("MUSICA_TEST_DIR") else { return };
        let lib = scan(Path::new(&dir));
        println!("{}", serde_json::to_string_pretty(&lib).unwrap());
        assert!(!lib.tracks.is_empty());
        assert!(lib.tracks.iter().all(|t| !t.file_name.starts_with('.')), "hidden files must be skipped");
    }
}
