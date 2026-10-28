use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use std::future::Future;
use std::time::{Duration, Instant};

use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::{OnceCell, Semaphore};

use super::{DownloadOptions, Event, ExtraFile, Observer, Outcome, Summary};
use crate::api::{ArtistAlbumsFilter, TidalApi};
use crate::config::{AudioFormat, CoverResource, M3uResource, MediaFilter, SinglesFilter};
use crate::error::{Error, Result};
use crate::ffmpeg::{self, Container};
use crate::metadata::{self, Cover, TrackTags};
use crate::models::{Album, CreditsEntry, ListedItem, MediaItem, Playlist, Track};
use crate::paths::{resolve_existing_path_case, with_suffix};
use crate::quality::{is_lossless_api_name, track_quality_label, video_quality_label, TrackQuality};
use crate::resource::{Resource, ResourceKind};
use crate::stream::{parse_track_stream, video_segments};
use crate::template::{format_path, TemplateData};

/// At most this many progress events per download task (~10 a second): front
/// ends redraw on each, and fast connections deliver thousands of chunks.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);
/// Playlist covers are limited to 1080px by Tidal.
const PLAYLIST_COVER_MAX: u32 = 1080;
/// At most this many genres are written to the GENRE tag.
const MAX_GENRES: usize = 3;

/// A cover shared by every track of an album, downloaded at most once.
struct SharedCover {
    cover: Cover,
    data: OnceCell<Option<Vec<u8>>>,
}

impl SharedCover {
    fn new(uid: &str, size: u32) -> Arc<Self> {
        Arc::new(Self { cover: Cover::new(uid, size), data: OnceCell::new() })
    }

    async fn data(&self, http: &reqwest::Client) -> Option<&[u8]> {
        self.data.get_or_init(|| self.cover.fetch(http)).await.as_deref()
    }
}

/// Album-level data written into every track's tags.
#[derive(Default, Clone)]
struct ItemMeta {
    date: Option<String>,
    album_artist: String,
    credits: Vec<CreditsEntry>,
    cover: Option<Arc<SharedCover>>,
    comment: String,
    genre: String,
    /// Album ReplayGain from the stream info (the track's own comes with the track).
    replay_gain: Option<crate::dsp::ReplayGain>,
}

impl ItemMeta {
    fn for_album(album: &Album) -> Self {
        Self {
            date: album.release_date.clone(),
            album_artist: album.artist.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
            ..Default::default()
        }
    }
}

struct Inner {
    api: TidalApi,
    http: reqwest::Client,
    opts: DownloadOptions,
    semaphore: Semaphore,
    observer: Arc<dyn Observer>,
    next_task: AtomicU64,
    summary: Mutex<Summary>,
    albums: Mutex<HashMap<i64, Album>>,
}

/// Downloads Tidal resources according to [`DownloadOptions`].
#[derive(Clone)]
pub struct Downloader {
    inner: Arc<Inner>,
}

type ItemResult = Option<(PathBuf, MediaItem)>;

impl Downloader {
    pub fn new(api: TidalApi, opts: DownloadOptions, observer: Arc<dyn Observer>) -> Self {
        let threads = opts.threads.max(1);
        Self {
            inner: Arc::new(Inner {
                http: api.client().http().clone(),
                api,
                opts,
                semaphore: Semaphore::new(threads),
                observer,
                next_task: AtomicU64::new(1),
                summary: Mutex::new(Summary::default()),
                albums: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn options(&self) -> &DownloadOptions {
        &self.inner.opts
    }

    /// Downloads every resource concurrently and returns the totals.
    pub async fn run(&self, resources: &[Resource]) -> Result<Summary> {
        let results = bounded(
            resources
                .iter()
                .map(|r| async move { self.handle_resource(r).await.inspect_err(|e| self.fail(r.to_string(), e)) })
                .collect(),
            self.fan_out(),
        )
        .await;

        if self.inner.opts.fail_fast {
            results.into_iter().collect::<Result<Vec<_>>>()?;
        }
        Ok(*self.inner.summary.lock().unwrap())
    }

    /// Items prepared at once: enough to keep every download slot busy while
    /// the next ones look for existing files, without polling a 2000-track
    /// playlist's items (and their API calls) all together.
    fn fan_out(&self) -> usize {
        self.inner.opts.threads.max(1) * 2
    }

    fn emit(&self, event: Event) {
        {
            let mut s = self.inner.summary.lock().unwrap();
            match &event {
                Event::Queued { .. } => s.queued += 1,
                Event::Finished { .. } => s.downloaded += 1,
                Event::Exists { .. } => s.existing += 1,
                Event::Skipped { .. } => s.skipped += 1,
                Event::Failed { .. } => s.failed += 1,
                _ => {}
            }
        }
        self.inner.observer.on_event(event);
    }

    fn fail(&self, context: impl Into<String>, error: &Error) {
        tracing::warn!("{error}");
        self.emit(Event::Failed { id: None, context: context.into(), message: error.humane() });
    }

    /// Reports `result`'s error and turns it into `None`, unless `fail_fast`.
    fn report<T>(&self, context: impl FnOnce() -> String, result: Result<T>) -> Result<Option<T>> {
        match result {
            Ok(v) => Ok(Some(v)),
            Err(e) if self.inner.opts.fail_fast => Err(e),
            Err(e) => {
                self.fail(context(), &e);
                Ok(None)
            }
        }
    }

    fn template<'a>(&'a self, configured: &'a str) -> &'a str {
        self.inner.opts.template_override.as_deref().unwrap_or(configured)
    }

    /// Quality name exposed to templates as `{item.quality}`.
    fn item_quality(&self, item: &MediaItem) -> String {
        let opts = &self.inner.opts;
        match item {
            MediaItem::Track(t) => {
                let q = if opts.track_quality == TrackQuality::Max
                    && !t.media_metadata.tags.iter().any(|t| t == "HIRES_LOSSLESS")
                {
                    TrackQuality::High
                } else {
                    opts.track_quality
                };
                q.as_str().to_uppercase()
            }
            MediaItem::Video(_) => opts.video_quality.as_str().to_uppercase(),
        }
    }

    async fn album_cached(&self, id: i64) -> Result<Album> {
        if let Some(album) = self.inner.albums.lock().unwrap().get(&id) {
            return Ok(album.clone());
        }
        let album = self.inner.api.album(&id.to_string()).await?;
        self.inner.albums.lock().unwrap().insert(id, album.clone());
        Ok(album)
    }

    /// Album data for an item, fetched only when the template needs it.
    async fn album_for_template(&self, template: &str, item: &MediaItem) -> Result<Option<Album>> {
        match item.album_id() {
            Some(id) if template.contains("{album") => self.album_cached(id).await.map(Some),
            _ => Ok(None),
        }
    }

    fn genre(genres: Vec<String>) -> String {
        genres.into_iter().take(MAX_GENRES).collect::<Vec<_>>().join(", ")
    }

    fn cover_for(&self, uid: Option<&str>, save_resource: CoverResource) -> Option<Arc<SharedCover>> {
        let opts = &self.inner.opts;
        let uid = uid?;
        (opts.metadata.cover || opts.cover.saves(save_resource)).then(|| SharedCover::new(uid, opts.cover.size))
    }

    async fn save_cover_file(&self, cover: &SharedCover, template: &str, data: TemplateData) -> Result<()> {
        if template.is_empty() {
            tracing::warn!("cover saving is enabled but no cover template is set");
            return Ok(());
        }
        let relative = format_path(template, &data, false)?;
        let path = self.inner.opts.download_dir.join(relative);
        let bytes = cover.data(&self.inner.http).await;
        if let Some(path) = cover.cover.save(&self.inner.http, bytes, &path).await? {
            self.emit(Event::ExtraWritten { kind: ExtraFile::Cover, path });
        }
        Ok(())
    }

    async fn save_m3u(
        &self,
        kind: M3uResource,
        template: &str,
        data: TemplateData,
        items: Vec<ItemResult>,
    ) -> Result<()> {
        if !self.inner.opts.m3u.saves(kind) || template.is_empty() {
            return Ok(());
        }
        let tracks: Vec<(PathBuf, Track)> = items
            .into_iter()
            .flatten()
            .filter_map(|(path, item)| match item {
                MediaItem::Track(t) => Some((path, t)),
                MediaItem::Video(_) => None,
            })
            .collect();
        let path = self.inner.opts.download_dir.join(format_path(template, &data, false)?);
        if let Some(path) = blocking(move || crate::m3u::save(&tracks, &path)).await?? {
            self.emit(Event::ExtraWritten { kind: ExtraFile::Playlist, path });
        }
        Ok(())
    }

    async fn handle_resource(&self, resource: &Resource) -> Result<()> {
        let api = &self.inner.api;
        let opts = &self.inner.opts;

        match resource.kind {
            ResourceKind::Track => {
                let track = api.track(&resource.id).await?;
                let album = match track.album.id {
                    Some(id) => Some(self.album_cached(id).await?),
                    None => None,
                };
                let item = MediaItem::Track(track.clone());

                let cover_uid = album.as_ref().and_then(|a| a.cover.as_deref()).or(track.album.cover.as_deref());
                let cover = self.cover_for(cover_uid, CoverResource::Track);

                let mut meta = album.as_ref().map(ItemMeta::for_album).unwrap_or_default();
                meta.genre = Self::genre(api.track_genres(track.id).await);
                meta.cover = cover.clone();

                let data = TemplateData::new(self.item_quality(&item)).item(&item).album_opt(album.as_ref());
                let path = format_path(self.template(opts.templates.track()), &data, true)?;
                self.handle_item(item, path, meta).await?;

                if let (true, Some(cover)) = (opts.cover.saves(CoverResource::Track), cover) {
                    self.save_cover_file(&cover, &opts.cover.templates.track, data).await?;
                }
            }

            ResourceKind::Video => {
                let video = api.video(&resource.id).await?;
                let item = MediaItem::Video(video);
                let template = self.template(opts.templates.video());
                let album = self.album_for_template(template, &item).await?;
                let data = TemplateData::new(self.item_quality(&item)).item(&item).album_opt(album.as_ref());
                let path = format_path(template, &data, true)?;
                self.handle_item(item, path, ItemMeta::default()).await?;
            }

            ResourceKind::Album => {
                let album = api.album(&resource.id).await?;
                self.download_album(&album).await?;
            }

            ResourceKind::Artist => self.download_artist(&resource.id).await?,

            ResourceKind::Mix => {
                let template = self.template(opts.templates.mix());
                let mut futures = Vec::new();
                let mut offset = 0;
                loop {
                    let page = api.mix_items(&resource.id, offset).await?;
                    for listed in &page.items {
                        let prepared = self
                            .prepare_listed(template, listed, |data| data.extra("mix_id", resource.id.as_str()))
                            .await;
                        if let Some((item, path, meta)) = self.report(|| listed_context(listed), prepared)? {
                            futures.push(self.handle_item(item, path, meta));
                        }
                    }
                    match page.next_offset() {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
                let items = collect(bounded(futures, self.fan_out()).await, opts.fail_fast)?;
                let data = TemplateData::new("").extra("mix_id", resource.id.as_str()).extra("type", "mix");
                self.save_m3u(M3uResource::Mix, &opts.m3u.templates.mix, data, items).await?;
            }

            ResourceKind::Playlist => {
                let playlist = api.playlist(&resource.id).await?;
                self.download_playlist(&playlist).await?;
            }
        }
        Ok(())
    }

    /// Fetches what a listed (mix / playlist) item needs and renders its path.
    async fn prepare_listed(
        &self,
        template: &str,
        listed: &ListedItem,
        decorate: impl FnOnce(TemplateData) -> TemplateData,
    ) -> Result<(MediaItem, String, ItemMeta)> {
        let item = listed.item.clone();
        let album = self.album_for_template(template, &item).await?;

        let mut meta = album.as_ref().map(ItemMeta::for_album).unwrap_or_default();
        if let MediaItem::Track(t) = &item {
            meta.genre = Self::genre(self.inner.api.track_genres(t.id).await);
        }

        let data = decorate(TemplateData::new(self.item_quality(&item)).item(&item).album_opt(album.as_ref()));
        let path = format_path(template, &data, true)?;
        Ok((item, path, meta))
    }

    async fn download_album(&self, album: &Album) -> Result<()> {
        let api = &self.inner.api;
        let opts = &self.inner.opts;
        let album_id = album.id.to_string();

        let cover = self.cover_for(album.cover.as_deref(), CoverResource::Album);

        let review = if opts.metadata.album_review {
            match api.album_review(&album_id).await {
                Ok(r) => r.normalized_text(),
                Err(e) => {
                    tracing::debug!("no review for album {}: {e}", album.id);
                    String::new()
                }
            }
        } else {
            String::new()
        };
        let genre = Self::genre(api.album_genres(album.id).await);
        let template = self.template(opts.templates.album());

        let mut futures = Vec::new();
        let mut offset = 0;
        loop {
            let page = api.album_items_credits(&album_id, offset).await?;
            let next_offset = page.next_offset();
            for listed in page.items {
                let data = TemplateData::new(self.item_quality(&listed.item)).item(&listed.item).album(album);
                let path = match self.report(|| listed_context(&listed), format_path(template, &data, true))? {
                    Some(p) => p,
                    None => continue,
                };

                let mut credits = listed.credits;
                metadata::normalize_credits(&mut credits);
                let meta = ItemMeta {
                    credits,
                    cover: cover.clone(),
                    comment: review.clone(),
                    genre: genre.clone(),
                    ..ItemMeta::for_album(album)
                };
                futures.push(self.handle_item(listed.item, path, meta));
            }
            match next_offset {
                Some(next) => offset = next,
                None => break,
            }
        }

        let items = collect(bounded(futures, self.fan_out()).await, opts.fail_fast)?;

        let data = TemplateData::new("").album(album).extra("type", "album");
        self.save_m3u(M3uResource::Album, &opts.m3u.templates.album, data, items).await?;

        if let (true, Some(cover)) = (opts.cover.saves(CoverResource::Album), cover) {
            let data = TemplateData::new("").album(album);
            self.save_cover_file(&cover, &opts.cover.templates.album, data).await?;
        }
        Ok(())
    }

    async fn download_artist(&self, artist_id: &str) -> Result<()> {
        let api = &self.inner.api;
        let opts = &self.inner.opts;

        let mut video_futures = Vec::new();
        if opts.videos_filter != MediaFilter::None {
            let template = self.template(opts.templates.video());
            let mut offset = 0;
            loop {
                let page = api.artist_videos(artist_id, offset).await?;
                for video in page.items.iter().cloned() {
                    let item = MediaItem::Video(video);
                    let prepared = async {
                        let album = self.album_for_template(template, &item).await?;
                        let data = TemplateData::new(self.item_quality(&item)).item(&item).album_opt(album.as_ref());
                        format_path(template, &data, true)
                    }
                    .await;
                    let context = || format!("Video: {} ({})", item.title(), item.id());
                    if let Some(path) = self.report(context, prepared)? {
                        video_futures.push(self.handle_item(item, path, ItemMeta::default()));
                    }
                }
                match page.next_offset() {
                    Some(next) => offset = next,
                    None => break,
                }
            }
        }

        let mut albums = Vec::new();
        if opts.videos_filter != MediaFilter::Only {
            let filters: &[ArtistAlbumsFilter] = match opts.singles_filter {
                SinglesFilter::None => &[ArtistAlbumsFilter::Albums],
                SinglesFilter::Only => &[ArtistAlbumsFilter::EpsAndSingles],
                SinglesFilter::Include => &[ArtistAlbumsFilter::Albums, ArtistAlbumsFilter::EpsAndSingles],
            };
            for filter in filters {
                let mut offset = 0;
                loop {
                    let page = api.artist_albums(artist_id, offset, *filter).await?;
                    albums.extend(page.items.iter().cloned());
                    match page.next_offset() {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
            }
        }

        let album_futures = albums.iter().map(|album| async move {
            let result = self.download_album(album).await;
            self.report(|| format!("Album: {} ({})", album.title, album.id), result).map(drop)
        });

        // albums one slot each: their tracks fan out on their own
        let album_futures: Vec<_> = album_futures.collect();
        let (videos, albums) = futures::join!(
            bounded(video_futures, self.fan_out()),
            bounded(album_futures, self.inner.opts.threads.max(1))
        );
        collect(videos, opts.fail_fast)?;
        albums.into_iter().collect::<Result<Vec<_>>>()?;
        Ok(())
    }

    async fn download_playlist(&self, playlist: &Playlist) -> Result<()> {
        let api = &self.inner.api;
        let opts = &self.inner.opts;
        let template = self.template(opts.templates.playlist());

        let mut futures = Vec::new();
        let mut index = 0;
        let mut offset = 0;
        loop {
            let page = api.playlist_items(&playlist.uuid, offset).await?;
            for listed in &page.items {
                index += 1;
                let prepared = self.prepare_listed(template, listed, |data| data.playlist(playlist, index)).await;
                if let Some((item, path, meta)) = self.report(|| listed_context(listed), prepared)? {
                    futures.push(self.handle_item(item, path, meta));
                }
            }
            match page.next_offset() {
                Some(next) => offset = next,
                None => break,
            }
        }

        let items = collect(bounded(futures, self.fan_out()).await, opts.fail_fast)?;

        let data = TemplateData::new("").playlist(playlist, 0).extra("type", "playlist");
        self.save_m3u(M3uResource::Playlist, &opts.m3u.templates.playlist, data, items).await?;

        if let (true, Some(uid)) = (opts.cover.saves(CoverResource::Playlist), playlist.square_image.as_deref()) {
            let cover = SharedCover::new(uid, opts.cover.size.min(PLAYLIST_COVER_MAX));
            let data = TemplateData::new("").playlist(playlist, 0);
            self.save_cover_file(&cover, &opts.cover.templates.playlist, data).await?;
        }
        Ok(())
    }

    /// Downloads one item and writes its tags. `relative` is the rendered
    /// template ending with `.*`.
    async fn handle_item(&self, item: MediaItem, relative: String, mut meta: ItemMeta) -> Result<ItemResult> {
        self.emit(Event::Queued { id: item.id().to_string(), title: item.title().to_owned() });

        let result = match self.download(&item, Path::new(&relative)).await {
            Err(e) if !self.inner.opts.fail_fast => {
                let context = format!("{}: {} ({})", item_kind(&item), item.title(), item.id());
                tracing::warn!("{context}: {e}");
                self.emit(Event::Failed { id: Some(item.id().to_string()), context, message: e.humane() });
                None
            }
            other => other?,
        };
        let Some((path, downloaded, replay_gain)) = result else {
            return Ok(None);
        };
        meta.replay_gain = replay_gain;

        let opts = &self.inner.opts;
        if opts.metadata.enable && (downloaded || opts.rewrite_metadata) {
            let result = self.write_tags(&item, &path, meta).await;
            self.report(|| format!("Tags: {}", path.display()), result)?;
        }

        if opts.update_mtime && !downloaded {
            let file = path.clone();
            let touched = blocking(move || {
                std::fs::File::options().append(true).open(&file)?.set_modified(std::time::SystemTime::now())
            })
            .await;
            if let Err(e) = touched.and_then(|r| r.map_err(Error::from)) {
                tracing::warn!("could not update mtime of {}: {e}", path.display());
            }
        }

        Ok(Some((path, item)))
    }

    async fn write_tags(&self, item: &MediaItem, path: &Path, meta: ItemMeta) -> Result<()> {
        let opts = &self.inner.opts;
        match item {
            MediaItem::Track(track) => {
                let lyrics = if opts.metadata.lyrics || opts.write_lrc_file {
                    match self.inner.api.track_lyrics(track.id).await {
                        Ok(l) => l.subtitles.filter(|s| !s.trim().is_empty()).or(l.lyrics),
                        Err(e) => {
                            tracing::debug!("no lyrics for {}: {e}", track.id);
                            None
                        }
                    }
                } else {
                    None
                };

                if opts.write_lrc_file {
                    if let Some(lrc) = lyrics.as_deref().filter(|l| !l.trim().is_empty()) {
                        let lrc_path = with_suffix(path, "lrc");
                        tokio::fs::write(&lrc_path, lrc).await?;
                        self.emit(Event::ExtraWritten { kind: ExtraFile::Lyrics, path: lrc_path });
                    }
                }

                let mut cover = meta.cover;
                if cover.is_none() && opts.metadata.cover {
                    cover = track.album.cover.as_deref().map(|uid| SharedCover::new(uid, opts.cover.size));
                }
                let cover_data = match (&cover, opts.metadata.cover) {
                    (Some(c), true) => c.data(&self.inner.http).await.map(<[u8]>::to_vec),
                    _ => None,
                };

                let tags = TrackTags {
                    album_artist: meta.album_artist,
                    date: meta.date,
                    lyrics: if opts.metadata.lyrics { lyrics } else { None },
                    credits: meta.credits,
                    cover: cover_data,
                    comment: meta.comment,
                    genre: meta.genre,
                    ..TrackTags::from_track(track)
                };
                let tags = TrackTags {
                    replay_gain: match (meta.replay_gain, tags.replay_gain) {
                        (Some(stream), Some(listed)) => Some(stream.or(listed)),
                        (a, b) => a.or(b),
                    },
                    ..tags
                };
                let path = path.to_path_buf();
                spawn_blocking(move || metadata::write_track_tags(&path, &tags)).await
            }
            MediaItem::Video(video) => {
                if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp4")) {
                    return Ok(()); // .ts files (no ffmpeg) can not be tagged
                }
                let (path, video) = (path.to_path_buf(), video.clone());
                spawn_blocking(move || metadata::write_video_tags(&path, &video)).await
            }
        }
    }

    /// Returns the final path and whether it was (re)downloaded, or `None`
    /// when the item was skipped.
    async fn download(
        &self,
        item: &MediaItem,
        relative: &Path,
    ) -> Result<Option<(PathBuf, bool, Option<crate::dsp::ReplayGain>)>> {
        let opts = &self.inner.opts;

        if !item.allow_streaming() {
            self.emit(Event::Skipped {
                id: item.id().to_string(),
                title: item.title().to_owned(),
                reason: "not available for streaming".into(),
            });
            return Ok(None);
        }

        let filename = match item {
            MediaItem::Track(track) => match opts.audio_format.extension() {
                Some(ext) => with_suffix(relative, ext),
                None => with_suffix(relative, predicted_extension(track, opts.track_quality)),
            },
            MediaItem::Video(_) => with_suffix(relative, "mp4"),
        };

        // every file system look-up of the item in one trip to the blocking pool
        let (existing, exists, target) = {
            let (scan_dir, download_dir, name) = (opts.scan_dir.clone(), opts.download_dir.clone(), filename.clone());
            let match_case = opts.match_existing_path_case;
            blocking(move || {
                let existing = resolve_path(match_case, &scan_dir, &name);
                let exists = existing.exists();
                let target = if scan_dir == download_dir {
                    existing.clone()
                } else {
                    resolve_path(match_case, &download_dir, &name)
                };
                (existing, exists, target)
            })
            .await?
        };
        let outcome = if exists {
            if opts.skip_existing {
                self.emit(Event::Exists {
                    id: item.id().to_string(),
                    title: item.title().to_owned(),
                    path: existing.clone(),
                });
                return Ok(Some((existing, false, None)));
            }
            Outcome::Overwritten
        } else {
            let filtered = match item {
                MediaItem::Video(_) => opts.videos_filter == MediaFilter::None,
                MediaItem::Track(_) => opts.videos_filter == MediaFilter::Only,
            };
            if filtered {
                let reason = format!("videos filter is `{:?}`", opts.videos_filter).to_lowercase();
                self.emit(Event::Skipped { id: item.id().to_string(), title: item.title().to_owned(), reason });
                return Ok(None);
            }
            Outcome::Downloaded
        };

        let _permit = self.inner.semaphore.acquire().await.expect("semaphore is never closed");

        let mut replay_gain = None;
        let (urls, mut path, quality, extract_flac) = match item {
            MediaItem::Track(track) => {
                let stream = self.inner.api.track_stream(track.id, opts.track_quality).await?;
                let atmos = stream.audio_mode == "DOLBY_ATMOS";
                let filtered = match opts.atmos_filter {
                    MediaFilter::None => atmos,
                    MediaFilter::Only => !atmos,
                    MediaFilter::Allow => false,
                };
                if filtered {
                    let reason = format!("Dolby Atmos filter is `{:?}`", opts.atmos_filter).to_lowercase();
                    self.emit(Event::Skipped { id: item.id().to_string(), title: item.title().to_owned(), reason });
                    return Ok(None);
                }

                let source = parse_track_stream(&stream)?;
                replay_gain = Some(crate::dsp::ReplayGain::from_stream(&stream)).filter(|r| !r.is_empty());
                let path = target.clone();

                if is_lossless_api_name(&stream.audio_quality) && !atmos {
                    let mut quality = track_quality_label(&stream.audio_quality).to_owned();
                    if let (Some(bits), Some(rate)) = (stream.bit_depth, stream.sample_rate) {
                        quality = format!("{quality} {bits}-bit {:.1} kHz", rate as f64 / 1000.0);
                    }
                    (source.urls, path, quality, true)
                } else {
                    let quality = if atmos {
                        "Dolby Atmos".to_owned()
                    } else {
                        track_quality_label(&stream.audio_quality).to_owned()
                    };
                    (source.urls, with_suffix(&path, "m4a"), quality, false)
                }
            }
            MediaItem::Video(video) => {
                let stream = self.inner.api.video_stream(video.id, opts.video_quality).await?;
                let urls = video_segments(&self.inner.http, &stream).await?;
                let path = with_suffix(&target, "ts");
                (urls, path, video_quality_label(&stream.video_quality).to_owned(), false)
            }
        };

        let task = self.inner.next_task.fetch_add(1, Ordering::Relaxed);
        let title = item.title().to_owned();
        self.emit(Event::Started { task, id: item.id().to_string(), title: title.clone(), quality: quality.clone() });

        self.fetch_to_file(task, &urls, &path).await?;
        path = self.post_process(item, path, extract_flac).await;

        self.emit(Event::Finished { task, id: item.id().to_string(), title, quality, path: path.clone(), outcome });
        Ok(Some((path, true, replay_gain)))
    }

    /// Streams `urls` (in order) into `path` through a temporary file.
    async fn fetch_to_file(&self, task: u64, urls: &[String], path: &Path) -> Result<()> {
        let dir = path.parent().unwrap_or(Path::new("."));
        tokio::fs::create_dir_all(dir).await?;

        let tmp = tempfile::Builder::new().prefix(".homefi-").suffix(".part").tempfile_in(dir)?;
        let (std_file, tmp_path) = tmp.into_parts();
        let mut file = tokio::fs::File::from_std(std_file);

        let mut total = None;
        let mut downloaded = 0u64;
        let mut reported = Instant::now();
        for url in urls {
            let res = self.inner.http.get(url).send().await?.error_for_status()?;
            if urls.len() == 1 {
                total = res.content_length().filter(|l| *l > 0);
            }
            let mut body = res.bytes_stream();
            while let Some(chunk) = body.next().await {
                let chunk = chunk?;
                file.write_all(&chunk).await?;
                downloaded += chunk.len() as u64;
                if reported.elapsed() >= PROGRESS_EVERY {
                    reported = Instant::now();
                    self.inner.observer.on_event(Event::Progress { task, downloaded, total });
                }
            }
        }
        file.flush().await?;
        drop(file);
        self.inner.observer.on_event(Event::Progress { task, downloaded, total: total.or(Some(downloaded)) });

        tmp_path.persist(path).map_err(|e| Error::Io(e.error))?;
        crate::auth::set_mode(path, 0o644);
        Ok(())
    }

    /// Unwraps FLAC, remuxes videos and transcodes as configured. Failures
    /// are reported but keep the downloaded file.
    async fn post_process(&self, item: &MediaItem, path: PathBuf, extract_flac: bool) -> PathBuf {
        let opts = &self.inner.opts;
        let ffmpeg = ffmpeg::available().await;
        let title = item.title().to_owned();
        let mut path = path;

        match item {
            MediaItem::Track(_) if extract_flac => {
                // Hi-Res comes as FLAC inside MP4: remux it losslessly in Rust (no ffmpeg needed)
                let source = path.clone();
                let remuxed = tokio::task::spawn_blocking(move || crate::remux::flac_from_mp4(&source))
                    .await
                    .unwrap_or_else(|e| Err(Error::Stream(e.to_string())));
                match remuxed {
                    Ok(Some(p)) => path = p,
                    // native FLAC (Lossless) or AAC inside MP4: name it after what it is
                    Ok(None) => {
                        let file = path.clone();
                        let ext = match blocking(move || ffmpeg::sniff_container(&file)).await {
                            Ok(Container::Flac) => "flac",
                            _ => "m4a",
                        };
                        let target = with_suffix(&path, ext);
                        if target != path && tokio::fs::rename(&path, &target).await.is_ok() {
                            path = target;
                        }
                    }
                    Err(e) if ffmpeg => {
                        tracing::warn!("remux failed ({e}), trying ffmpeg");
                        match ffmpeg::extract_flac(&path).await {
                            Ok(p) => path = p,
                            Err(e) => self.fail(format!("Track: {title}"), &e),
                        }
                    }
                    Err(e) => {
                        self.fail(format!("Track: {title}"), &e);
                        let target = with_suffix(&path, "m4a");
                        if target != path && tokio::fs::rename(&path, &target).await.is_ok() {
                            path = target;
                        }
                    }
                }
            }
            MediaItem::Video(_) if ffmpeg => match ffmpeg::convert_to_mp4(&path).await {
                Ok(p) => path = p,
                Err(e) => self.fail(format!("Video: {title}"), &e),
            },
            _ => {}
        }

        if matches!(item, MediaItem::Track(_)) && opts.audio_format != AudioFormat::Original {
            if ffmpeg {
                match ffmpeg::convert_audio(&path, opts.audio_format).await {
                    Ok(p) => path = p,
                    Err(e) => self.fail(format!("Convert: {title}"), &e),
                }
            } else if path
                .extension()
                .is_none_or(|e| Some(e.to_string_lossy().as_ref()) != opts.audio_format.extension())
            {
                self.fail(format!("Convert: {title}"), &Error::Ffmpeg("ffmpeg is required to convert audio".into()));
            }
        }
        path
    }
}

/// Extension the downloaded file will most likely have, used to detect
/// existing files before asking for the stream.
fn predicted_extension(track: &Track, quality: TrackQuality) -> &'static str {
    if quality.is_lossless() && is_lossless_api_name(&track.audio_quality) {
        "flac"
    } else {
        "m4a"
    }
}

fn item_kind(item: &MediaItem) -> &'static str {
    match item {
        MediaItem::Track(_) => "Track",
        MediaItem::Video(_) => "Video",
    }
}

fn listed_context(listed: &ListedItem) -> String {
    let item = &listed.item;
    let mut context = format!("{}: {} ({})", item_kind(item), item.title(), item.id());
    if let Some(album) = item.album_id() {
        context.push_str(&format!(", album {album}"));
    }
    context
}

fn collect(results: Vec<Result<ItemResult>>, fail_fast: bool) -> Result<Vec<ItemResult>> {
    if fail_fast {
        return results.into_iter().collect();
    }
    Ok(results.into_iter().map(|r| r.ok().flatten()).collect())
}

/// Joins `base` and `relative`, reusing the casing of existing folders when
/// asked (blocking: it reads directories).
fn resolve_path(match_case: bool, base: &Path, relative: &Path) -> PathBuf {
    if match_case {
        resolve_existing_path_case(base, relative)
    } else {
        base.join(relative)
    }
}

/// Runs `futures` at most `limit` at a time; the results keep the input order
/// (playlist files list tracks in order).
async fn bounded<F: Future>(futures: Vec<F>, limit: usize) -> Vec<F::Output> {
    let mut done: Vec<(usize, F::Output)> =
        futures::stream::iter(futures.into_iter().enumerate().map(|(i, f)| async move { (i, f.await) }))
            .buffer_unordered(limit.max(1))
            .collect()
            .await;
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, out)| out).collect()
}

/// Blocking file-system work off the async workers.
async fn blocking<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::Io(std::io::Error::other(e.to_string())))
}

async fn spawn_blocking<F>(f: F) -> Result<()>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bounded_keeps_the_order_and_the_limit() {
        use std::sync::atomic::AtomicUsize;
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let futures: Vec<_> = (0..20u64)
            .map(|i| {
                let (running, peak) = (running.clone(), peak.clone());
                async move {
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20 - i)).await;
                    running.fetch_sub(1, Ordering::SeqCst);
                    i
                }
            })
            .collect();
        assert_eq!(bounded(futures, 4).await, (0..20).collect::<Vec<_>>());
        assert_eq!(peak.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn extension_prediction() {
        let track: Track =
            serde_json::from_value(serde_json::json!({"id": 1, "title": "x", "audioQuality": "LOSSLESS"})).unwrap();
        assert_eq!(predicted_extension(&track, TrackQuality::High), "flac");
        assert_eq!(predicted_extension(&track, TrackQuality::Normal), "m4a");

        let lossy: Track =
            serde_json::from_value(serde_json::json!({"id": 1, "title": "x", "audioQuality": "HIGH"})).unwrap();
        assert_eq!(predicted_extension(&lossy, TrackQuality::Max), "m4a");
    }
}
