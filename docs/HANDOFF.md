# HomeFi: contexto completo para seguir el trabajo

> Documento de traspaso: estábamos trabajando en una Mac y seguimos en otra
> computadora con otro agente. Aquí está la idea, el estado de cada pieza, lo
> que falta, cómo montar el entorno y las reglas que el dueño (José) pidió.
> Léelo completo antes de tocar código. Última actualización: 2026-10-27.

---

## 1. La idea

**HomeFi** es un reproductor y descargador de música Hi-Fi personal, hecho en
casa. Empezó como reescritura en Rust de [tiddl](https://github.com/oskvr37/tiddl)
(descargador de Tidal en Python) y creció a un ecosistema propio:

- **CLI `homefi`**: compatible con tiddl (mismo config, plantillas de rutas y
  flags), descarga tracks, álbumes, playlists, mixes y videos de Tidal.
- **App de escritorio** (macOS, Windows, Linux): Tauri 2 + Svelte 5. Biblioteca
  local, Tidal (buscar, explorar, streaming, favoritos, playlists), descargas y
  la **"sala de escucha"** (Room): portada con halo, cadena de señal, vúmetros,
  visualizador y una **mascota** (una casita con audífonos) que baila según el
  género y el ritmo de la canción.
- **App Android** (Kotlin + Jetpack Compose): lo mismo en el teléfono, con
  reproductor Media3, layouts de reproductor, skins retro, Home con recientes,
  descargas, Tidal y la mascota.
- **Reproductor Astell&Kern SR35** (DAP sin Google Play ni adb): la app Android
  se instala "disfrazada" en el hueco de Spotify de OpenService (ver §7).

La motivación de Rust (palabras de José): *"la idea de usar rust era para la
app de mac osx, windows, linux"*. Un solo núcleo (`homefi-core`) sirve a la
CLI, al escritorio y, vía UniFFI, a Android.

### Lo que viene: "modo carro" y "modo fiesta"

- **Modo carro / control compartido** (en curso): el teléfono de José es el
  **centro** (está conectado al carro y reproduce). Los pasajeros, todos con
  Android, abren HomeFi en su teléfono, se unen sin internet ni router
  (Wi-Fi Direct), **buscan, ven la cola, agregan canciones** y, si el centro lo
  permite, saltan, pausan o reordenan. El audio nunca sale del centro.
  José insistió: **el camino principal es la app Android nativa del pasajero**,
  no una página web; la página web servida por el centro es solo un respaldo
  (para quien no tiene HomeFi o tiene iPhone).
  Diseño completo: [`docs/shared-control.md`](shared-control.md).
- **Modo fiesta** (después, reutiliza todo lo del modo carro): el centro tiene la
  música y la transmite; cada invitado la recibe, guarda 1–2 s en RAM y la toca
  sincronizada en su propia bocina Bluetooth. FLAC por Wi-Fi Direct u Opus por
  Bluetooth, calibración de latencia por bocina (manual o con micrófono), y
  roles por bocina (izquierda/derecha/sub).

---

## 2. Reglas y filosofía (obligatorio)

1. **Hablar en español** con José.
2. **Todo es configurable, en teléfono y escritorio.** Palabras de José:
   *"que todo sea configurable en phone y en desktop todo porque es nuestra
   filosofia"*. Cada aspecto visual o de comportamiento nuevo es un ajuste, y
   existe en ambas plataformas (si algo existe solo en una, es un pendiente).
3. **Público desde la v1, con cuidado.** Lo único que se publica son los
   instaladores que genera el workflow de release con un tag `vX.Y.Z`, y solo
   cuando José lo pide. Nunca se publica el APK del SR35 (usa el id de Spotify),
   ni llaves, sesiones o datos personales. El aviso de uso personal y no
   comercial se acepta antes de conectar Tidal (README, NOTICE y las apps).
4. **Nunca imprimir tokens.** La sesión de Tidal vive en `~/.homefi/auth.json`.
   Contra la cuenta real solo se hacen pruebas de **lectura**; las escrituras
   (favoritos, playlists) se probaron solo con wiremock.
5. **Borrar = mover a la Papelera**, y solo archivos de la biblioteca.
6. **El APK de Android sale del CI** firmado con la llave de release (secrets
   `ANDROID_*`). La llave vive fuera del repo, en `~/.homefi-release/`; si se
   pierde, nadie puede actualizar sin desinstalar.
7. **Sin certificado de Apple**: la app de macOS se firma ad-hoc (`"-"`).
8. Sin marcas ajenas: las skins retro imitan el estilo, nunca nombres ni logos.
9. Commits: autor `Jose España <51006254+joseespana@users.noreply.github.com>`,
   mensaje en inglés estilo `Area: what changed`, con el trailer
   `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Push directo a
   `main` (José lo eligió así).
10. Antes de declarar algo terminado: `make check` (fmt + clippy `-D warnings` +
    tests), `pnpm check` en el escritorio y compilación de Kotlin.

---

## 3. Mapa del repositorio

```
homefi/
├── Cargo.toml                 # workspace; la versión de todo vive aquí (0.1.0)
├── crates/
│   ├── homefi-core/           # Tidal (auth, API, cache), streams, plantillas, tags,
│   │                          # motor de descargas, biblioteca, m3u, stream_cache,
│   │                          # remux FLAC-en-MP4, flac_tags, share/ (modo carro)
│   ├── homefi-player/         # motor de audio (symphonia + cpal + rubato), FFT
│   ├── homefi-cli/            # comando `homefi`
│   └── homefi-ffi/            # UniFFI para Android (lib.rs + share.rs)
├── apps/
│   ├── desktop/               # Tauri 2 + Svelte 5 (src/ = UI, src-tauri/ = Rust)
│   ├── android/               # Kotlin + Compose (usa homefi-ffi)
│   └── ios/                   # SwiftUI (usa homefi-ffi como XCFramework), ver docs/ios.md
├── xtask/                     # cargo xtask icons | bump | version
├── assets/brand/              # logo/mascota (build_icon.py genera SVG/PNG)
├── tools/sr35/                # utilidades MTP en C para subir el APK al SR35
├── docs/                      # config, plantillas, releasing, shared-control, este archivo
├── .claude/agents, skills     # agentes: rust-pro, kotlin-specialist, mobile-developer,
│                              # fullstack-developer; skills: rust-pro, mobile-design, ui-ux-pro-max
└── .github/workflows/         # ci.yml + release.yml
```

---

## 4. Estado por componente

### Núcleo Rust (`homefi-core`)
- Auth por dispositivo, `TidalClient` con caché en disco, `TidalApi` (búsqueda,
  álbum/artista/playlist, top tracks, favoritos, playlists propias, crear/agregar/
  quitar/borrar playlist con ETag, favoritos add/remove, tracks por ISRC).
  Límite de página de favoritos: 50 (Tidal rechaza más).
- Descargas con eventos (la CLI y las apps los pintan). `remux.rs` saca FLAC del
  MP4 bit a bit; `flac_tags.rs` escribe metadatos sin tocar el audio (lofty
  corrompía frames). Portada embebida por defecto.
- `library.rs` (escaneo con caché), `artwork.rs` (portadas faltantes vía Tidal),
  `m3u.rs` (playlists locales con rutas relativas), `stream_cache.rs`.
- **`share/`** (modo carro, fase 1, ver §6).
- Hallazgo: Tidal entrega LOSSLESS 16/44.1 aunque se pida HI_RES (límite del
  cliente/plan, igual que tiddl).

### Reproductor (`homefi-player`)
Cola, gapless a igual frecuencia, remuestreo sinc a la mejor frecuencia del
dispositivo, shuffle/repeat, `append`/`move_track`, niveles y bandas FFT para
los visuales. Probado en la Mac con jack a 96 kHz y Bluetooth a 44.1.

### Escritorio (`apps/desktop`)
- Vistas: Room, canciones, álbumes, artistas, búsqueda, Mi Tidal, detalle de
  Tidal, carpeta, Playlists (.m3u locales y de Tidal), descargas, ajustes.
- Streaming de Tidal con caché (2 adelante, tope 3 GB), cola con "reproducir
  a continuación", mini reproductor siempre visible, notificaciones flotantes,
  estados de descarga por fila, corazón y "agregar a playlist", borrar a la
  Papelera con animación "Thanos", barras de carga y skeletons.
- Ajustes: tema Sistema/Claro/Oscuro, 6 estilos de barra de progreso (+ grosor
  y animación), calidad, carpetas, paneles de la sala, mascota.
- **Mascota** (`Mascot.svelte`, `lib/dance.ts`, `lib/mood.ts`): estados de ánimo
  por género (mosh, pogo, cute, party, salsa, groove, chill, bop) con disfraces;
  brazos y piernas; bailes famosos (jagger, moonwalk, twist, robot, disco, ymca,
  floss, macarena, running, horse, guitar). Click = baile, arrastrar = moverla
  (flotante, en cualquier pantalla), rueda = tamaño, doble click = regresarla.
  Todo en ajustes (`MascotOptions.svelte`).
- **Control compartido y fiesta en escritorio** (2026-10-27): centro e invitado
  en la red local (sin Wi-Fi Direct ni Bluetooth), anuncio y búsqueda por mDNS
  (`_homefi._tcp`, mismos TXT que Android), QR, código, invitados, aprobaciones,
  quitar con "bloquear" sin marcar, modo fiesta como centro (archivo actual y
  siguiente, espera a que todos estén listos, niveles) y como invitado (segundo
  motor, caché `party-cache` con descarga por rangos, sincronía **solo con
  saltos**: el motor de escritorio no cambia la velocidad). Vista de invitado
  con Player / Room / Search; el deck y el mini reproductor muestran la canción
  de la sala. Ajustes → "Shared control & party" con todo lo de Android menos
  las radios del teléfono. Detalles: `docs/shared-control.md` → "Desktop".
  Probado: pruebas de Rust (sincronía, TXT, búsqueda local, invitado completo
  contra un centro real en 127.0.0.1), la app real en esta Mac como centro con
  un invitado de prueba (mDNS, `/hello`, búsqueda en Tidal, agregar, descarga
  adelantada, quitar) y capturas con el mock. **Sin probar**: contra un centro o
  invitado Android real, la sincronía de oído entre dos computadoras/bocinas,
  Windows y Linux (compila con crates puros, falta correrlo ahí).
- Mock para desarrollo sin Tauri: `src/lib/mock.ts` (parámetros de URL
  `?static`, `?slow`, `?float`, `?theme=light|dark`, y para compartir `?hub`,
  `?guest`, `?party`, `?nearby`, `?playback`).
- Sin probar con mouse real: arrastre, rueda y doble click de la mascota.

### Android (`apps/android`)
- Pestañas: Home (recientes, continuar), biblioteca (incluye .m3u), búsqueda,
  Tidal (colección, páginas de artista/álbum/playlist, streaming con caché 2 GB),
  descargas, ajustes.
- Reproductor: 8 layouts (CLASSIC, IMMERSIVE, VINYL, STUDIO, RETRO_AMP, LUNA,
  ALUMINUM, WEB2000), fondo con portada difuminada, cola con portadas, espectro,
  vúmetros, barras de progreso configurables, selector de salida de audio.
- Mascota portada 1:1 del escritorio (`ui/player/Dance.kt`, `Mood.kt`,
  `Mascot.kt`): tap, arrastrar, pellizcar, doble tap; opciones en Ajustes →
  Reproductor.
- Login dentro de la app (WebView) + QR. Selector de carpeta con soporte de SD.
- Decodificador FFmpeg de Jellyfin para ALAC (GPL-3.0 → el APK es GPL, anotado
  en NOTICE/README).
- **Sin probar en dispositivo real**: botones multimedia, selector de salida,
  la mascota nueva, el build release con R8.

### iPhone (`apps/ios`)
- SwiftUI sobre el mismo núcleo (XCFramework + bindings Swift de UniFFI).
  Se instala solo a mano desde la Mac con Xcode; detalles en [`docs/ios.md`](ios.md).
- Hecho: biblioteca, Tidal (login, favoritos, búsqueda, streaming),
  reproductor en segundo plano, mascota con giros, ajustes, EN/ES.
- Falta para igualar Android: descargas, páginas de Tidal, .m3u, Home, layouts
  y skins, análisis de audio para la mascota, modo carro.

### CLI
`auth`, `download` (flags de tiddl), `config`, `devices`, `play`, búsqueda.

### CI / releases
- `ci.yml`: Rust en Linux/macOS/Windows + frontend; **todavía compila un APK
  debug** (José no ha dicho si quitarlo).
- `release.yml`: en push a main, tags `v*` y manual. Instaladores de escritorio
  (dmg arm64/x64, Windows x64/arm64, Linux deb/rpm/AppImage x64/arm64) y CLI para
  6 targets; job de Android comentado; subida opcional a Cloudflare R2.
  Detalles en [`docs/releasing.md`](releasing.md).
- Ojo: los runners Linux arm64 pueden no estar disponibles en repos privados.

---

## 5. Commits hasta ahora

```
9b30c07 Core: shared control hub and guest (share module) with FFI
aa27f1c Docs: shared control (car mode) and the hub design
a603950 Android: mascot with arms and legs, genre dances and famous dances
3c3473b Desktop: mascot with arms and legs, famous dances, drag it anywhere
9462916 Desktop: selectable seek bars, same set as Android
3a346d0 Android: selectable seek bars and a Player settings section
2ef7731 Android: four retro player skins and the blurred album backdrop
0db17b2 Android: Home with recently played, covers in the queue
f0ed6dc Android: four player layouts, even spacing, and a queue
a09fe5c Desktop: play next / add to queue
...      (ver git log; el primero grande es 4fd62e9)
```

---

## 6. Modo carro: dónde vamos y qué sigue

Diseño: [`docs/shared-control.md`](shared-control.md). Resumen técnico:

- Conexión: grupo **Wi-Fi Direct** con el centro como dueño del grupo;
  descubrimiento por servicio `_homefi._tcp` (banner "José está compartiendo
  música · Unirme"); QR como respaldo. Protocolo WebSocket JSON en el puerto
  **47800** + HTTP para portadas (`/cover/<id>`) y la página web (`/`).
- Seguridad: código aleatorio de 4 dígitos; bloqueo de IP tras 5 códigos malos;
  ping/timeout; mensajes de máximo 64 KiB.

### Fase 1, núcleo: HECHA (`9b30c07`)
- `crates/homefi-core/src/share/{protocol,permissions,queue,hub,guest}.rs`,
  pruebas en `tests/share.rs` (28 unitarias + 8 de integración).
- Feature `share` (axum 0.8 con ws, tokio-tungstenite 0.29 sin TLS): solo la
  activa el FFI; la CLI no carga esas dependencias.
- FFI (`crates/homefi-ffi/src/share.rs`):
  - Centro: `startHub(HubSettings, HubHostCallback) -> ShareHub`. El callback
    (lo implementa Kotlin) responde `search`, `browse`, `cover` y recibe
    `onCommand` (skip/toggle/volume/play que pidieron los pasajeros) y `onEvent`
    (alguien entró/salió, pidió agregar, etc.). `ShareHub` tiene `info()`
    (incluye `joinUri` para el QR), `state`, `guests`, `setPermissions`, `kick`,
    `add`, `remove`, `moveItem`, `pending`/`approve`/`reject`,
    `setNowPlaying`, `setPosition`, `setQueue`, `setVolume`, `notify`, `stop`.
  - Pasajero: `joinHub(...) -> ShareGuest` (eventos por `GuestListener`), con
    `search`, `browse`, `add`, `remove`, `moveItem`, `skip`, `voteSkip`,
    `toggle`, `volume`, `resolveUrl`, `disconnect`.
  - `parseJoinUri`, `shareDefaultPermissions`.
- Permisos por defecto: agregar libre (máx. 3 por persona), orden **por turnos**
  (una canción de cada quien por ronda; el centro cuenta como una persona),
  saltar **por votación** (la mitad), pausa/volumen no, quitar/mover solo las
  propias, explícito permitido. El centro puede expulsar.

### Modo fiesta (bocinas) y fase 2/3 en Android: ESCRITO, falta probar en teléfonos (2026-10-26)
- Núcleo: `share/sync.rs` (reloj tipo NTP, posición esperada, acción de
  sincronía: ±4 % de velocidad o salto), `clock`/`audio` en el protocolo,
  `GET /audio/<id>?k=<llave>` con Range, `Hub::set_audio/set_streaming/now_ms`,
  `drop_songs_on_leave`, "a continuación" real del centro. Diseño en
  `docs/shared-control.md` → "Party mode".
- Android (`share/`, `share/net/`, `ui/share/`): centro (Compartir control o
  Fiesta, Wi-Fi Direct o Wi-Fi de casa, QR, código, invitados, aprobaciones,
  avisos discretos, servicio en primer plano), invitado (banner en Home,
  "Unirme con código", enlace `homefi://join` desde la cámara, pantalla remota,
  "Sonar en este teléfono" con retraso por bocina), todo en Ajustes →
  "Control compartido y fiesta".
- Probado en el emulador: centro en Wi-Fi de casa + invitado de prueba desde
  la Mac (unirse, búsqueda, reloj a 3 ms, audio por rangos). Sin probar:
  Wi-Fi Direct real, la sincronía de oído con bocinas Bluetooth, la pantalla
  del invitado (necesita dos teléfonos).
- Escritorio: hecho en la red local (ver §4, Escritorio). Falta iPhone.

### Decisiones por defecto (aceptadas: José dijo "arranca", 2026-10-26)
1. Si un pasajero se va, **sus canciones se quedan**; hacerlo ajuste
   ("quitar sus canciones al salir").
2. El centro tendrá un **"a continuación" real que se salta los turnos**
   (para pasajeros solo reordena lo suyo). Agregarlo en fase 2.
3. Si el centro reordena a mano bajo "turnos", el orden por turnos se reaplica
   en el siguiente cambio (para orden manual fijo, usar "por llegada").
4. Expulsar = cerrar el WebSocket con razón (no hay mensaje `kicked`). El ban es
   por IP; más adelante mandar un id de dispositivo en `hello`.
5. Kotlin debe pasar la IP del dueño del grupo Wi-Fi Direct como
   `advertiseHost`.

### Fase 2, Android centro: SIGUIENTE
- Botón **"Compartir control"** en el reproductor → arranca el hub (en un
  servicio, ligado al `PlaybackService`), muestra **QR** + código de 4 dígitos.
- Lista de pasajeros (color, nombre, expulsar/bloquear), solicitudes pendientes
  si "el centro aprueba".
- **Permisos en Ajustes** (todos configurables, regla §2.2) + ajuste "quitar sus
  canciones al salir".
- **Avisos discretos** que no distraigan al conductor ("Ana agregó X").
- Conectar con `PlayerController`: espejo de la cola y lo que suena →
  `setQueue`/`setNowPlaying`/`setPosition`; `onCommand` → acciones del player;
  `search`/`browse`/`cover` → Tidal + biblioteca local.
- Wi-Fi Direct: crear grupo y anunciar `_homefi._tcp`. Permisos Android:
  `NEARBY_WIFI_DEVICES` (API 33+), ubicación fina por debajo de 33.
  minSdk sigue en 24 para el SR35.
- Strings en EN y ES.

### Fase 3, Android pasajero
Banner de descubrimiento, unirse (o escanear QR), **modo remoto**: la barra de
"sonando" y la cola muestran al centro, y Búsqueda/Tidal/Biblioteca ganan
"Agregar a la cola de José". El pasajero puede mandar ids de su propio Tidal.
Necesita **dos teléfonos reales** (el emulador no tiene Wi-Fi Direct real).

### Fase 4 y 5
Página web de respaldo y centro en escritorio (la Mac no habla Wi-Fi Direct;
usar la red local). Luego subidas de canciones del pasajero y el modo fiesta.

---

## 7. Astell&Kern SR35 (sin adb)

- El firmware solo instala APKs de OpenService que usan el **package id de una
  app conocida**, desde su archivo en `OpenService/Download/` por MTP. HomeFi
  ocupa el hueco de **Spotify** (`Spotify.apk`); el `Tidal.apk` original quedó
  restaurado en su hueco.
- Build (desde `apps/android`):
  ```bash
  ./gradlew assembleRelease -Phomefi.abis=arm64-v8a,armeabi-v7a \
    -Phomefi.applicationId=com.spotify.music -Phomefi.minSdk=24 \
    -Phomefi.compressLibs=true -Phomefi.versionCode=<n>
  ```
  Salida: `app/build/outputs/apk/release/homefi-0.1.0-release.apk`.
- **versionCode**: el último subido al SR35 es **1000000017** (letras y karaoke, errores humanos, diseño en toda la app, carga por páginas, transiciones con el ajuste Movimiento); el próximo,
  1000000018.
- **FIRMA, IMPORTANTE**: sin keystore de release, el APK release se firma con la
  llave debug de la máquina (`~/.android/debug.keystore`). La app del SR35 está
  firmada con la llave debug de **la Mac vieja**. Para poder actualizarla desde
  la computadora nueva hay que **copiar `~/.android/debug.keystore` de la Mac
  vieja a la nueva** (mismo path). Si no, Android rechaza la actualización por
  firma distinta y habría que desinstalar (se pierden los ajustes de la app).
  Lo mismo aplica a cualquier teléfono con la versión debug instalada.
- Subir al SR35 (modo USB Disk/MTP; Finder no habla MTP):
  ```bash
  brew install libmtp
  cd tools/sr35 && for t in mtpswap mtpput mtpls mtpget; do
    clang -o $t $t.c -I$(brew --prefix)/include -L$(brew --prefix)/lib -lmtp; done
  F=$(mtp-folders 2>/dev/null | awk '/OpenService$/{o=1;next} o&&/^[0-9]+\t  Download$/{print $1;exit} /^[0-9]+\t[^ ]/{o=0}')
  ./mtpswap $F Spotify.apk /tmp/Spotify-prev.apk <apk nuevo>
  ```
  `mtpswap` copia el archivo viejo (a `/tmp`, no hace falta guardarlo), verifica el tamaño, lo borra y sube el
  nuevo en una sola sesión (el SR35 rechaza consultas por carpeta, por eso se
  usa el listado completo). El id de carpeta cambia entre conexiones.
- En el reproductor: logo **A** → Services → Spotify → Install.
- No se guardan respaldos de los APK originales (Spotify, Tidal): José los
  vuelve a descargar desde Services en el propio reproductor si hace falta.
- Pantalla del SR35: emulador `homefi_sr35` (720×1280, densidad 320).

---

## 8. Montar el entorno en la computadora nueva

Clonar:
```bash
git clone git@github.com:joseespana/homefi.git
```
Para firmar releases a mano hace falta la llave de `~/.homefi-release/` (no está
en el repo; José la guarda aparte). La llave debug de Android
(`~/.android/debug.keystore`) es la que firmó la app instalada en el SR35.

Requisitos:
- Rust (rustup; `rust-toolchain.toml` fija el canal) y targets
  `aarch64-linux-android armv7-linux-androideabi`; `cargo install cargo-ndk`.
- JDK 21, Android SDK con platform 36, **NDK 29.0.14206865**, emulador.
  Exportar `ANDROID_HOME` y `ANDROID_NDK_HOME` antes de gradle.
- Node 22 + pnpm 11 (`packageManager: pnpm@11.10.0`).
- `ffmpeg` (opcional; lo usan algunas pruebas de tags), `libmtp` para el SR35.
- Linux: `libasound2-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev`.

Comandos:
```bash
make check                               # fmt + clippy + tests (debe pasar)
cd apps/desktop && pnpm install && pnpm tauri dev   # app de escritorio
pnpm check                               # tipos de Svelte
cd apps/android && ./gradlew -q compileDebugKotlin -Phomefi.skipRust=true  # Kotlin rápido
make android-debug                       # APK debug arm64 (regenera bindings UniFFI)
cargo run -p homefi-cli -- auth login    # sesión de Tidal (~/.homefi/auth.json)
```
- Capturas del escritorio sin Tauri: `pnpm dev` y Chrome headless contra
  `http://localhost:1420/?static#room` (el mock carga solo fuera de Tauri).
- Emulador tamaño SR35:
  ```bash
  sdkmanager "system-images;android-36;google_apis;arm64-v8a"
  avdmanager create avd -n homefi_sr35 -k "system-images;android-36;google_apis;arm64-v8a"
  # luego en ~/.android/avd/homefi_sr35.avd/config.ini: hw.lcd.width=720, hw.lcd.height=1280, hw.lcd.density=320
  emulator -avd homefi_sr35 -no-audio -gpu swiftshader_indirect
  ```
  Paquete debug: `com.perseusoft.homefi.debug`. Poner FLACs de prueba en
  `/sdcard/Music/Test`.
- Los bindings Kotlin de UniFFI se generan desde la librería **debug del host**
  (`target/debug/libhomefi_ffi.*`); los `.so` release van sin símbolos.

---

## 9. Pendientes

- [ ] **v1.0.0 pública**: secrets `ANDROID_*` en GitHub, repo en público
      (lo cambia José), luego `cargo xtask bump 1.0.0` y el tag `v1.0.0`.
- [ ] Respuesta de José a las decisiones por defecto del modo carro (§6).
- [ ] **Fase 2** del modo carro (Android centro). Luego fase 3 (pasajero) con
      dos teléfonos reales.
- [x] Subir al SR35 un APK nuevo: 1000000009 con la mascota que gira (2026-10-26).
- [x] Subir al SR35 1000000010 con el modo fiesta (2026-10-26).
- [x] Subir al SR35 1000000011: descubrimiento sin cámara ni GPS, Bluetooth LE, ajustes en secciones, registro de errores en Android/data/com.spotify.music/files/logs (2026-10-26).
- [ ] Probar en dispositivo real: botones multimedia, selector de salida,
      mascota, build R8.
- [ ] Probar con mouse real la mascota flotante del escritorio.
- [ ] Decidir si `ci.yml` deja de compilar el APK debug.
- [ ] Filosofía "todo configurable": en escritorio faltan equivalentes de los
      layouts/skins del reproductor Android, de Home/recientes y un fondo
      difuminado configurable; vista previa de la mascota en Ajustes de Android.
- [ ] Con la mascota flotante, los vúmetros de la sala se separan un poco.

---

## 10. Cómo se ha trabajado

- José pide en español, a veces en pocas palabras; se le responde en español,
  claro y sin jerga, y con capturas cuando hay UI nueva.
- Trabajo grande se reparte con los agentes de `.claude/agents/`: `rust-pro`
  (núcleo/FFI), `kotlin-specialist` (pantallas Android), `mobile-developer`
  (permisos, servicios, firma, APK), `fullstack-developer` (una función de punta
  a punta).
- Cada función nueva: escritorio y Android a la par, en Ajustes, con strings
  EN/ES, verificada (tests, `pnpm check`, capturas headless, emulador).
