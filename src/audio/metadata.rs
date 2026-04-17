use std::path::Path;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey};
use symphonia::core::probe::Hint;

#[derive(Debug, Default, Clone)]
pub struct AudioMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

pub fn read(path: &Path) -> AudioMetadata {
    let Ok(file) = std::fs::File::open(path) else {
        return AudioMetadata::default();
    };
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let Ok(mut probed) = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) else {
        return AudioMetadata::default();
    };

    let mut out = AudioMetadata::default();

    // Container-level metadata (e.g. ID3 v2 in mp3 wrapper).
    if let Some(rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        merge_tags(&mut out, rev.tags());
    }
    // Stream-level metadata (e.g. Vorbis comments in ogg/flac).
    if let Some(rev) = probed.format.metadata().current() {
        merge_tags(&mut out, rev.tags());
    }

    out
}

fn merge_tags(out: &mut AudioMetadata, tags: &[symphonia::core::meta::Tag]) {
    for tag in tags {
        let value = tag.value.to_string().trim().to_string();
        if value.is_empty() {
            continue;
        }
        match tag.std_key {
            Some(StandardTagKey::TrackTitle) if out.title.is_none() => {
                out.title = Some(value);
            }
            Some(StandardTagKey::Artist) if out.artist.is_none() => {
                out.artist = Some(value);
            }
            Some(StandardTagKey::AlbumArtist) if out.artist.is_none() => {
                out.artist = Some(value);
            }
            Some(StandardTagKey::Album) if out.album.is_none() => {
                out.album = Some(value);
            }
            _ => {}
        }
    }
}
