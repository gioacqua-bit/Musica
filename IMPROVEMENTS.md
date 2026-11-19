# Musica: planned improvements

Requested improvements, to be implemented in Musica. New items are appended in the order they are given.

| # | Improvement | Status | Version |
|---|---|---|---|
| 1 | Playlists | Implemented | v0.2.0 |

## Version history

| Version | Date | Changes |
|---|---|---|
| v0.2.0 | 2026-11-19 | Playlists (item 1) |
| v0.1.0 | 2026-11-18 | First release: library by artist, player, light/dark themes, Aizawa visualizer |

## 1. Playlists

**Status:** implemented in v0.2.0.

Requirements:

- **Playlist creation:** create a new, empty playlist, to be filled afterwards.
- **Playlist deletion:** erase an existing playlist.
- **Multiple membership:** a song can belong to one or more playlists at the same time.
- **Adding songs:** a method to add specific songs to a playlist.
  - The same method must also allow creating a new playlist on the spot, with the selected song(s) added to it.

Implementation (v0.2.0):

- **Sidebar:** a *Playlists* section above *Artists*, with a **+** button that creates an empty playlist (also <kbd>Ctrl/⌘+N</kbd>) and opens it.
- **Playlist page:** cover mosaic, song count and duration; **Play**, **Shuffle**, **Rename**, **Delete** (with confirmation; music files are never touched); each song shows artist · album and can be removed with **×**.
- **Add-to-playlist menu:** opened from the button next to each track, the button next to each album title (adds the whole album), the button next to the playing song, or by right-clicking a track.
  - Lists every playlist with a tick for those already containing the song; clicking toggles membership, so a song can be put in several playlists from one menu.
  - **New playlist…** at the bottom creates a playlist with the selected song(s) in it.
  - A song is never added twice to the same playlist.
- **Storage:** `playlists.json` in the app's data folder (Windows `%APPDATA%\io.github.musica.player`, macOS `~/Library/Application Support/io.github.musica.player`, Linux `~/.local/share/io.github.musica.player`), written atomically. Songs are referenced by file path, so playlists survive rescans and folder changes; songs not in the current music folder are shown greyed out and skipped during playback.
