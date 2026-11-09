# Android TV / Google TV: UI spec

Status: design for phase 1 of `docs/TV_APP_PLAN.md` (shell, login, player) and the screens of phases 2 to 4.
Visual reference: `docs/tv-mockup.html` (static, 1920 x 1080; keys 1 to 9 switch screens, D opens the drawer;
`tv-mockup.html#player`). Code lives in `apps/android/.../tv/*` on `androidx.tv:tv-material`.

## 1. Rationale

- **Same family as the ride page and the phone, not stock Material.** Glass cards, ambient light, one vivid
  accent gradient (blue to teal, the logo and the progress bars), big confident type. The ride page's zone light
  becomes the cover's light here: the backdrop is the cover's dominant colour, blurred once (not per frame).
- **10 feet, one hand, no pointer.** One focused thing per screen, always obvious. Nothing is hover-only, nothing
  needs a long reading distance, every action has a key.
- **Focus is a shape, never only a colour.** Focus = scale up + white fill or white ring + accent ring + glow +
  (cards) the label drops 14 px. A colour-blind user and a washed-out TV both still see it.
- **Calm by default.** Dark, low-saturation backdrops; the accent appears only on the playing thing, the focused
  thing and progress. A living room at night must not glare.
- **Reuse, don't fork.** `Skin` still colours the app (accent, shapes); the TV layout is one layout. Settings
  hold every new look (rule: every look is a setting, desktop and Android; TV ones live in `AppSettings.tv`).

## 2. Units, safe area, type

The TV is a 960 x 540 dp screen at 1080p (1 dp = 2 px), 1920 x 1080 px; 4K draws the same dp. The mockup is in px.

| Token | dp | px @1080p | Notes |
| --- | --- | --- | --- |
| Safe margins | 48 side / 27 top-bottom | 96 / 54 (5 %) | `TvDimens.safe`; setting `tv.safeMargins` off removes it |
| Gap between cards | 16 | 32 | row and column |
| Radius | card 12, pill full, panel 14 | 24 / full / 28 | skins may override the radius, never the focus rules |
| Min focus target | 36 high | 72 | pills, rows 48 dp (96 px) |

Type (system sans; skins may swap the family). **Floor: nothing under 12 sp (24 px). Body is 16 sp (32 px)**, which
is the plan's rule and covers the brief's 24 px minimum at 1080p. All times `tv.textScale` 0.9 to 1.3.

| Role | sp (px) | Weight | Use |
| --- | --- | --- | --- |
| Display | 36 (72) | 800 | login title, ambient clock |
| Screen title | 32 (64) | 800 | "Library", "Settings" |
| Section / song | 20 (40) | 700 | row titles, player title 30 sp |
| Body | 16 (32) | 600 | drawer items, buttons, rows |
| Card title | 15 (30) | 650 | under cards |
| Secondary | 12 (24) | 500 | subtitles, hints, tags (caps 700, tracking 0.16em) |

Karaoke line 32 sp (64 px) 800, neighbours 17 sp. Contrast: text on glass at least 4.5:1 (secondary `#a9b6cb` on `#0b1020`
is 8:1); focus white on dark 17:1. Light mode (setting `Appearance`, TV defaults dark): background `#f3f5fa`, glass becomes
white at 70 %, focus fill is the dark ink with a white label, ring and glow keep the accent.

## 3. Colour tokens (dark default)

`bg #060810`, `bg2 #0b1020`, ink `#f4f7fb`, ink2 `#a9b6cb`, ink3 `#6f7d94`, accent gradient `#4f8cff` to `#22d3c5`
(skin accent when a skin sets one), ok `#34d399`, warn `#fbbf24`, error `#f87171`, glass = white 9 % to 3 % diagonal,
1 px white 10 % border, inner top highlight. Backdrop = bg gradient + two radial lights (accent top-right 30 %,
teal bottom-left 16 %); on the player the main light follows the cover.

## 4. Navigation drawer

`NavigationDrawer` (tv-material). Two states, one component:

- **Collapsed rail** (default, content has focus): 72 px icons, logo mark on top, the current section marked by an
  accent bar on the left edge and a tinted circle behind its icon. Takes 168 px + margin; content starts at 264 px.
- **Open** (D-pad LEFT from the first column of any list, or BACK once at the top level): 640 px, icon + label,
  opaque `#070a14` fading out; content behind dims to 60 % and loses focus. Focus item = white pill + accent ring +
  glow, scale 1.04. UP/DOWN move, OK enters the section and collapses the drawer; BACK collapses.
- Order: Home, Library, Search, TIDAL, Downloads, Playlists, Sound, Party, Train, DJ mode, Settings. First focus on a cold
  start: Home content, not the drawer. TIDAL is hidden until signed in (the Login screen shows instead of Home).
- **DJ mode**: dimmed with a badge "needs a mouse" when no `SOURCE_MOUSE` device is connected (OK explains it in a
  dialog); normal when a mouse exists (setting `tv.djEntry`).
- Badges: Downloads shows the active count, Party shows the guest count when the hub runs.

## 5. Focus language (all components)

| Component | Focused | Selected / current | Pressed (OK) | Disabled |
| --- | --- | --- | --- | --- |
| Card (cover) | scale 1.08, 5 px white ring + 4 px accent ring + 60 px accent glow, label down 14 px | n/a | scale 1.03 for 80 ms | 40 % opacity |
| Pill / button | white fill, dark label, accent ring, glow, scale 1.06 | accent gradient fill (primary) | scale 1.02 | 40 %, still focusable if it explains why |
| List row | white fill, dark text, ring, scale 1.02 | title in teal + animated bars icon (playing) | same | dim |
| Switch | row focused as list row; switch gradient when on | on = gradient + knob right | toggles | dim |
| Value row | row focused; `◀ value ▶` arrows shown only when focused; LEFT/RIGHT change | n/a | OK opens a list if > 5 values | dim |
| Text field | white fill, caret, mic button next to it | n/a | OK opens the TV keyboard | dim |

Rules: never drop focus (no focus on a disabled-and-hidden control; `focusRestorer` returns to the last item when you come
back); every list restores its last position; focus never lands on text. Long press OK on any card or row opens the context
menu (play, play next, queue, download, heart, add to playlist, Sound for this). A mouse hover shows the same ring (a mouse can
be plugged for DJ). Motion setting `OFF`: no scale animation, instant ring; `SUBTLE`: scale 1.04, no glow pulse.

## 6. Screens

### 6.1 Home
Header: greeting caps + "Continue listening" (screen title), right: quality badge (TIDAL HiRes, FLAC), output badge
("HDMI · system mix", the honest word for HDMI), clock. Hero glass card (236 px high): cover 176 px, "Playing now",
title 46 px, artist, progress, primary "Open player" focused first when something plays; otherwise the hero is "Pick
something to play" with the last played. Rows below, one per line, 230 px covers: Recently played, Made for tonight (mixes,
playlists, sound presets), Your TIDAL, What's new (release notes card). Rows scroll vertically with the focused row pinned at 40 %
of the height; the last row ends at the safe margin. States: empty (no library, no account) shows the empty state of section 6.9.

### 6.2 Library
Title + count ("2 184 songs · 3 volumes"). View pills (Albums, Songs, Artists, Folders, Playlists .m3u) switched with LEFT/RIGHT
when the pill row has focus; volume chip on the right (USB badge). Grid 6 x n of 236 px covers (artist: circle, folder: glass tile
with folder icon, songs: list rows). 2 000 files scroll at 60 fps: `LazyVerticalGrid`, covers decoded at card size, 
placeholders are the skeleton. A-Z jump: hold UP/DOWN on the pill row opens a letter rail on the right edge.

### 6.3 Search
Left: query field (mic button, TV keyboard on OK, voice on mic), recent searches as chips. Right: results grouped (Songs list, Albums
row, Artists row, Playlists row). Empty query shows recents + "Try: songs by name, artist, album". No results: "Nothing for
"..."" with the query and "Check the spelling or try fewer words". Error (offline): the error state with "Try again".

### 6.4 TIDAL
Sub-pills: Collection (hearts), Playlists, Albums, Artists, Mixes. Artist/album/playlist pages: banner glass with cover 280 px, title 40 sp,
primary Play, Shuffle, Heart, Download; track list below as rows with the playing one in teal. Signed out: this section is the login screen.

### 6.5 Now Playing (full screen)
Layout (1920 x 1080 within the safe area):

- Left 620 px: cover (36 px radius, accent light behind it), title 60 px, artist and album 34 px.
- Top left: three tags: source + format ("TIDAL · FLAC 24/96"), output ("HDMI · system mix"), EQ preset. Top right: clock.
- Right glass panel (926 x 590): tabs **Lyrics, Homifito (mascot), Visualizer, Queue**. Lyrics: the current line 64 px 800 white with the
  teal karaoke fill from the left as it is sung, neighbours 34 px dimmed; the panel scrolls by line. Mascot and visualizer reuse the same
  `AudioTap`. Queue: rows with cover thumbs, OK plays, long OK menu. Default tab `tv.playerPanel`.
- Under the panel: progress bar (12 px, gradient, glowing thumb), elapsed and remaining, controls row: previous, play/pause (128 x 96
  pill, primary), next, heart, shuffle, Sound. Controls hide after `tv.controlsHideSeconds` (5) leaving cover, title and panel; any key brings them back.
- Backdrop: the cover's dominant colour as a blurred radial light (blur radius 24 dp, computed once per song).
- Skins keep their colours (`tv.playerLayoutColours`); no skin changes the layout.

**Key map**

| Key | Controls hidden | Controls visible |
| --- | --- | --- |
| OK | play / pause | activates the focused control |
| LEFT / RIGHT | previous / next; hold = seek by `tv.dpadSeekSeconds` (10), with a seek badge | move focus |
| UP | switch panel tab (cycles) | focus the tab strip |
| DOWN | show controls | focus the panel (scroll lines) |
| Long OK | track context menu | same |
| BACK | close the player (return to the section) | hide controls first, then close |
| MEDIA_PLAY_PAUSE, NEXT, PREVIOUS, FAST_FORWARD, REWIND | session as usual; FF / REW seek by `dpadSeekSeconds` | same |
| MENU / long BACK | opens queue | same |

States: buffering (spinner in the cover corner + "Buffering…", progress bar shimmer), no lyrics ("No lyrics for this song" with a
"Search lyrics" pill), error ("Can't play this song", Skip and Retry; auto-skip after 5 s setting is the existing playback setting),
nothing playing (player is not reachable; "Open player" replaced by Pick something).

### 6.6 Ambient / screensaver
After `tv.ambientSeconds` (default 60, 0 = off) of no key while playing: backdrop darkens to `#04050a` with the cover light, a
380 px cover with a 140 px glow, "Now playing", title 84 px, artist 42 px, thin progress (opacity 70 %) and, per `tv.ambientStyle`, the
karaoke line (56 px), Homifito, or a clock only. The whole group **drifts** to a new spot every 60 s (cross-fade 2 s, burn-in
protection); the clock top-right 72 px weight 300; party code bottom-right when a party runs. Any key wakes and is not
forwarded as an action (first key only wakes). Paused longer than `tv.awakeAfterPauseMinutes` releases keep-awake so the system
screensaver can take over. Motion setting `OFF`: no drift animation, a fixed block at 60 % opacity, moved by a step.

### 6.7 Login
Terms notice first (personal, non-commercial use; "I understand" pill focused). Then: logo + "Sign in to TIDAL from your phone", the
device code in eight large glass tiles (72 px, `ABCD · EFGH`), a QR (420 px, white tile for scanning contrast) next to it, "Get a new
code" pill (focused), countdown and status line ("Expires in 04:32 · waiting for you…"). On success the screen goes to Home with
the account name. Error (expired): code tiles dim, "The code expired" + new code pill; offline: error state. BACK exits the app
from here only. No in-app web login on TV.

### 6.8 Downloads
Two storage cards (focusable, one selected with the accent ring and "Selected" tag): Internal (free space bar, warn colour under
the minimum, text "1.2 GB free of 8 GB · low"), USB drive per volume ("688 GB free of 1 TB · writes to HomeFi/ on the drive"). A
target pill ("Save to: Internal / USB / Ask me"), Pause all, Clear finished. Queue list: cover, title, progress bar and the target
tag; failed rows show "Failed" + Retry; long OK cancels. States: no downloads (empty), USB removed (notice banner + rows marked
unavailable, nothing deleted), not enough room ("Not enough space on Internal. Choose the USB drive or free some space" with
both as pills). The minimum free space (`minFreeMb`, 500) is shown as a tag.

### 6.9 Settings (two-pane)
Left: section list (Appearance, Playback, Sound, Library, Downloads, TIDAL, Party, Train, TV, AI, About), 3 px accent selection
bar + tinted fill for the open section (not the white focus pill, which belongs to the focused item). Right: glass panel with the
section title, a one-line description ("Applies when HomeFi runs on a TV") and rows. Row kinds: **switch**, **value** (LEFT/RIGHT
cycles, OK opens a list for more than 5 options), **action** (OK). RIGHT from the list goes to the panel, LEFT/BACK returns.
Every field of `AppSettings` appears here (rule: configurable everything); TV section rows: screensaver after / style, keep screen
on, skip seconds, player panel, text size, safe margins, voice input, show DJ mode, download target, minimum free space, library
volumes, stream cache. Changes apply live; no Save button.

### 6.10 Party hub
Header "Party · hub on this TV" + the party name 64 px. Actions: Pause party (focused), Auto-approve, End party. Guest list: avatar,
name, last action ("Added Dreams · just now"), status tag; a pending guest shows Approve (primary) and Deny. Right glass card with
the QR (380 px), the code in a gradient 72 px ("HF-4821") and "Same Wi-Fi · open HomeFi, tap Join". The QR and code also show in
ambient mode. Wi-Fi only: Bluetooth and Wi-Fi Direct are hidden, with a tag that says so. Empty (no guests yet): "Nobody has joined
yet. Scan the code to start." Joining as a guest: a "Join a party" screen with the code tiles and keyboard.

### 6.11 Other sections (brief)
- **Playlists**: grid of playlists + "New with AI" card (chat with mic and quick replies chips 24 px high rows of 72 px).
- **Sound**: the EQ curve across the full width, band left/right, up/down changes `tv.eqStepDb`; presets row; the assistant with the mic.
  "Output: HDMI · system mix" says the truth (not bit-perfect).
- **Train**: two cards: "Watch a ride from your phone" and "Start a ride here" (setup chips); the ride itself is the core's own web page.
- **DJ mode**: with a mouse the phone's DJ screen at 1920 x 1080; without: glass card "DJ mode needs a mouse (USB or Bluetooth)".

### 6.12 Empty, loading, error
Same glass card (500 x 640): icon in accent line style, title 40 px, one sentence 28 px, one primary pill focused. Loading = skeleton rows
or cards with a slow shimmer (1.6 s, off when motion is `OFF`); never a lone spinner on a full screen. Errors name the thing and say
what is safe ("14 songs are unavailable until you plug it back in. Nothing was deleted."); the error card has a 1 px red border.

## 7. Motion

Follows `Motion` (AUTO / FULL / SUBTLE / OFF, `motionStyle` pace). FULL: focus scale 220 ms ease-out, ring cross-fade 160 ms, drawer
width 250 ms, page change cross-fade 200 ms with a 24 px slide, karaoke fill linear per word, mascot and visualizer from `AudioTap`,
ambient drift. SUBTLE: scale 1.04, no glow pulse, no drift (steps). OFF: no animation, focus is instant. `isLowRamDevice` maps AUTO to
SUBTLE, visualizer off, blur 12 dp. Only `transform` and `alpha` animate; no blur animation; no per-frame state: loops run while `STARTED`.

## 8. Per-skin differences

Skins re-tint accent, radius and surface, never focus semantics. Classic/Immersive: as the mockup. Vinyl/Studio: warm accent
(amber) and a darker glass. Retro amp / Luna / Aluminum / Web2000: keep their palette; their bevels apply to pills and cards as a
1 px bevel, focus is still scale + white fill + ring (verify contrast of the ring in light skins: use a dark ring `#0a1020` 3 px
inside the accent one). Motion pace follows the skin (stepped for Retro amp).

## 9. Accessibility

Focus visible on everything (shape + colour), order left to right then down, no traps (BACK always leaves), TalkBack labels on every
card ("Album Hurry Up, We're Dreaming, M83, button"), the playing item announced, mic hidden when no recognizer, minimum text 24 px,
text scale 0.9 to 1.3, never colour alone (status tags carry words), QR always paired with the typed code.

## 10. Copy (EN / ES)

| Key | EN | ES |
| --- | --- | --- |
| tv_home_continue | Continue listening | Sigue escuchando |
| tv_home_recent | Recently played | Escuchado hace poco |
| tv_home_tonight | Made for tonight | Para esta noche |
| tv_nav_* | Home, Library, Search, TIDAL, Downloads, Playlists, Sound, Party, Train, DJ mode, Settings | Inicio, Biblioteca, Buscar, TIDAL, Descargas, Listas, Sonido, Fiesta, Entrenar, Modo DJ, Ajustes |
| tv_dj_need_mouse | DJ mode needs a mouse (USB or Bluetooth) | El modo DJ necesita un ratón (USB o Bluetooth) |
| tv_login_title | Sign in to TIDAL from your phone | Inicia sesión en TIDAL desde tu móvil |
| tv_login_body | Scan the code, or open link.tidal.com on any device and type: | Escanea el código, o abre link.tidal.com en cualquier dispositivo y escribe: |
| tv_login_scan | Point your phone camera here | Apunta la cámara de tu móvil aquí |
| tv_login_new | Get a new code | Pedir un código nuevo |
| tv_login_expires | Expires in %1$s · waiting for you… | Caduca en %1$s · esperándote… |
| tv_login_expired | The code expired | El código ha caducado |
| tv_notice | Personal, non-commercial use. By continuing you accept the notice. | Uso personal y no comercial. Al continuar aceptas el aviso. |
| tv_np_open | Open player | Abrir reproductor |
| tv_np_tabs | Lyrics, Homifito, Visualizer, Queue | Letra, Homifito, Visualizador, Cola |
| tv_np_nolyrics | No lyrics for this song | Esta canción no tiene letra |
| tv_np_buffering | Buffering… | Cargando… |
| tv_np_error | Can't play this song | No se puede reproducir esta canción |
| tv_np_hint_ok | Play / pause | Reproducir / pausa |
| tv_np_hint_lr | Previous / next · hold seeks %1$d s | Anterior / siguiente · mantener salta %1$d s |
| tv_np_hint_up | Panel | Panel |
| tv_np_hint_down | Controls | Controles |
| tv_np_hint_back | Close | Cerrar |
| tv_amb_wake | Press any key to wake | Pulsa cualquier tecla para volver |
| tv_dl_internal | Internal storage | Almacenamiento interno |
| tv_dl_usb | USB drive | Unidad USB |
| tv_dl_free | %1$s free of %2$s | %1$s libres de %2$s |
| tv_dl_low | low | poco espacio |
| tv_dl_target | Save to: %1$s | Guardar en: %1$s |
| tv_dl_ask | Ask me | Preguntarme |
| tv_dl_nospace | Not enough space on %1$s. Choose another drive or free some space. | No hay espacio en %1$s. Elige otra unidad o libera espacio. |
| tv_dl_usb_gone | USB drive removed | Unidad USB desconectada |
| tv_dl_usb_gone_body | %1$d songs are unavailable until you plug it back in. Nothing was deleted. | %1$d canciones no están disponibles hasta que la vuelvas a conectar. No se ha borrado nada. |
| tv_dl_min | Minimum free space: %1$s | Espacio libre mínimo: %1$s |
| tv_set_tv_hint | Applies when HomeFi runs on a TV | Se aplica cuando HomeFi se ejecuta en una tele |
| tv_party_title | Party · hub on this TV | Fiesta · el centro está en esta tele |
| tv_party_join | Join this party | Únete a esta fiesta |
| tv_party_how | Same Wi-Fi · open HomeFi, tap Join | Misma Wi-Fi · abre HomeFi y toca Unirse |
| tv_party_empty | Nobody has joined yet. Scan the code to start. | Nadie se ha unido todavía. Escanea el código para empezar. |
| tv_party_lan | Wi-Fi only on a TV | En la tele solo Wi-Fi |
| tv_empty_title | Nothing here yet | Aún no hay nada |
| tv_empty_body | Add a folder or a USB drive and your music shows up here. | Añade una carpeta o una unidad USB y tu música aparece aquí. |
| tv_empty_cta | Add a music folder | Añadir una carpeta de música |
| tv_retry | Try again | Reintentar |
| tv_search_empty | Nothing for “%1$s” | Nada para «%1$s» |

## 11. Acceptance screenshots (Chromecast emulator `homefi_googletv`, 1920 x 1080; mockup shots in `tvui-shots/`)

1. Login (code + QR) and expired code. 2. Home with focus on the hero, then on a card; drawer open. 3. Library grid with focus,
and a volume chip. 4. Now Playing: lyrics tab (karaoke), mascot, visualizer, queue; controls hidden and visible. 5. Ambient after 60 s
and a wake. 6. Downloads with internal low + USB, and the USB removed state. 7. Settings two-pane on TV with a value row focused.
8. Party hub with QR. 9. Empty, loading, error. 10. Each of the above in light mode and with a light skin (Luna, Aluminum) for ring
contrast, text scale 1.3, motion OFF (no animation), and with an overscan margin on and off.
