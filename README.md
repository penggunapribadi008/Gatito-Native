# Gatito: Stream Tool 3.0.2, versi Rust (PC) + Kotlin (Android)

Ditulis ulang dari `Gatito Stream Tool v2.6.1` (Electron + Capacitor). Tujuannya 1:1: UI, fitur, dan logic sama,
bedanya hanya bahasa (Rust untuk PC, Kotlin untuk Android).

```
pc/        Rust. Server HTTP/WebSocket, mesin suara, OBS, VB-Cable, update, tray, dan GUI native (egui, tanpa HTML/WebView)
android/   Kotlin + Jetpack Compose. Aplikasi HP native (tanpa WebView), termasuk pemindai QR (CameraX)
.github/   build.yml: installer .exe (NSIS) dan .apk
```

## STATUS: BELUM DIKOMPILASI
Kode ditulis tanpa Rust toolchain, Android SDK, atau internet, jadi belum pernah di-build atau dijalankan.
Kemungkinan ada beberapa error kompilasi (nama API egui/eframe 0.29, versi crate). Kirim log error-nya.
Tampilan juga perlu dicek langsung dan disetel (jarak, ukuran font) karena egui dan Compose menggambar sedikit berbeda dari browser.

## Cara build
- PC (Windows): `cd pc && cargo build --release`. Installer: `makensis /DVERSION=3.0.2 installer.nsi`.
- Android: `cd android && bash build-apk.sh` (Gradle 8.7, JDK 17, Android SDK).
- GitHub: upload semua isi folder ini. Secrets `KEYSTORE_B64` dan `KEYSTORE_PASSWORD` sama seperti sebelumnya
  (alias `streamdeckhp`, `applicationId` `com.streamdeckhp.app`), jadi APK baru bisa menimpa yang lama.

## Peta berkas
| Versi Electron | Versi baru |
|---|---|
| `editor.html` | `pc/src/gui/` (mod.rs: editor, modals.rs: semua dialog, widgets.rs: komponen) |
| `vpad.html` | `pc/src/gui/vpad.rs` |
| `main.js` | `pc/src/main.rs`, `pc/src/shell.rs` (tray, jendela, keluar) |
| `server.js` | `pc/src/server.rs`, `pc/src/state.rs`, `pc/src/input.rs`, `pc/src/apps.rs`, `pc/src/update.rs` |
| `obs.js` | `pc/src/obs.rs` |
| `vbsetup.js` | `pc/src/winaudio.rs` (+ `pc/scripts/*.ps1`, dihasilkan dari kode asli) |
| `audio.html` | `pc/src/audio.rs` (cpal + symphonia) |
| `mobile/www/index.html` | `android/app/src/main/java/com/streamdeckhp/app/*.kt` |
| `MainActivity.java` | `MainActivity.kt`, `Updater.kt` |

GUI PC berbicara ke server lewat HTTP dan WebSocket yang sama dengan editor.html lama, jadi alurnya identik.

## Hal yang memang tidak bisa persis sama
- egui dan Compose bukan browser: anti-aliasing, kerning, dan pembulatan sedikit berbeda.
- Mesin suara memakai cpal + symphonia (bukan Web Audio): resample linear dan limiter peak sederhana pengganti DynamicsCompressor.
- Route `/editor`, `/vpad`, `/audiohost`, `/snd` dihapus (hanya dipakai jendela Electron). Ditambah `/__show` (lokal).
- `pc/assets/phone.html` masih ada: itu halaman web untuk HP yang membuka alamat PC lewat browser (bukan UI aplikasi).
