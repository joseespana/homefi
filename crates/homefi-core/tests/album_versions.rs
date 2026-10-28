//! Album versions against a mock Tidal server: an artist page as Tidal lists
//! it (Metallica, sanitized) grouped into cards, and the versions of one album.

use std::sync::Arc;

use homefi_core::album_versions::{Lang, VersionPrefs};
use homefi_core::api::{all_pages, ArtistAlbumsFilter, TidalApi, TidalClient};
use homefi_core::catalog;
use homefi_core::models::Album;
use serde_json::{json, Value};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn api(server: &MockServer) -> TidalApi {
    let base = server.uri();
    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    TidalApi::new(Arc::new(client), "1", "US")
}

fn fixture(filter: &str) -> Vec<Value> {
    let raw = include_str!("fixtures/metallica_albums.json");
    let all: Vec<Value> = serde_json::from_str(raw).unwrap();
    all.into_iter().filter(|v| v["filter"] == filter).collect()
}

async fn mount_artist(server: &MockServer) {
    for (filter, name) in [("ALBUMS", "ALBUMS"), ("EPSANDSINGLES", "EPSANDSINGLES")] {
        let items = fixture(filter);
        Mock::given(method("GET"))
            .and(path("/v1/artists/8405/albums"))
            .and(query_param("filter", name))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "limit": 100, "offset": 0, "totalNumberOfItems": items.len(), "items": items
            })))
            .mount(server)
            .await;
    }
}

#[tokio::test]
async fn an_artist_page_shows_one_card_per_album() {
    let server = MockServer::start().await;
    mount_artist(&server).await;
    let api = api(&server);
    let albums = all_pages(|o| api.artist_albums("8405", o, ArtistAlbumsFilter::Albums)).await.unwrap();
    assert_eq!(albums.len(), 43);

    let cards = catalog::album_cards(&albums, &VersionPrefs::default(), Lang::En);
    assert_eq!(cards.len(), 20);
    let reload = cards.iter().find(|c| c.versions.iter().any(|v| v.id == "536860794")).unwrap();
    assert_eq!(reload.id, "186460967", "original Reload, Hi-Res");
    assert!(reload.explicit);
    assert_eq!(reload.versions.len(), 5);
    assert_eq!(reload.versions.iter().filter(|v| v.primary).count(), 1);
    let atmos = reload.versions.iter().find(|v| v.atmos).unwrap();
    assert_eq!(atmos.label_codes, ["explicit", "dolby_atmos", "remastered"]);
    // single releases have no picker
    let lulu = cards.iter().find(|c| c.id == "34436068").unwrap();
    assert!(lulu.versions.is_empty());
    // the JSON leaves `versions` out when there are none
    assert!(serde_json::to_value(lulu).unwrap().get("versions").is_none());

    let flat = catalog::album_cards(&albums, &VersionPrefs { group: false, ..VersionPrefs::default() }, Lang::En);
    assert_eq!(flat.len(), 43);
}

#[tokio::test]
async fn an_album_knows_its_other_versions() {
    let server = MockServer::start().await;
    mount_artist(&server).await;
    let api = api(&server);
    let albums: Vec<Album> = fixture("ALBUMS").into_iter().map(|v| serde_json::from_value(v).unwrap()).collect();
    let load = albums.iter().find(|a| a.id == 441771365).unwrap();
    let versions = catalog::album_versions(&api, load, &VersionPrefs::default(), Lang::Es).await.unwrap();
    assert_eq!(versions.len(), 3);
    let me = versions.iter().find(|v| v.id == "441771365").unwrap();
    assert!(me.primary);
    assert_eq!(me.labels, ["Explícita", "Hi-Res", "Remasterizada"]);
    assert_eq!(me.description, "Explícita · Hi-Res 24-bit · Remasterizada · 14 canciones · 1996");

    // a single goes to the EPs & singles list
    let singles: Vec<Album> =
        fixture("EPSANDSINGLES").into_iter().map(|v| serde_json::from_value(v).unwrap()).collect();
    let lux = singles.iter().find(|a| a.id == 263412759).unwrap();
    let versions = catalog::album_versions(&api, lux, &VersionPrefs::default(), Lang::En).await.unwrap();
    assert_eq!(versions.len(), 2);
    assert!(versions.iter().any(|v| v.atmos && !v.primary));

    // an album the artist page doesn't list still has itself
    let mut lone = load.clone();
    lone.id = 1;
    lone.title = "Something Else".into();
    let versions = catalog::album_versions(&api, &lone, &VersionPrefs::default(), Lang::En).await.unwrap();
    assert_eq!(versions.len(), 1);
    assert!(versions[0].primary);
}
