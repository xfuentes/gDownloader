# Changelog

All notable changes to gDownloader are documented in this file.

## [Unreleased]

### Added
- Scripts page: JavaScript syntax highlighting in the script editor, and an
  "Example Scripts" menu offering JDownloader's own bundled example scripts
  (info file writer, play a sound, play a sound when inactive, reset a slow
  download) as a starting point.
- Settings sidebar: the Scripts tab now shows an enable/disable badge for
  the EventScripter extension itself, mirroring JDownloader's own settings
  tree.
- gDownloader now answers JDownloader's own dialogs (e.g. an EventScripter
  script asking to run an external program) with a native GTK Allow/Deny
  window instead of silently hanging — JDownloader runs headless, so it
  can't show these itself, but exposes them through its RemoteAPI for a
  client to answer on its behalf.
- JDownloader's "file already exists" dialog (for a plain download conflict
  or an archive-extraction conflict) now shows properly, with Skip/
  Overwrite/Rename choices, the relevant file/package/archive details, and
  a countdown, matching JDownloader's own dialog — previously it fell back
  to a generic Allow/Deny prompt that always silently skipped the file
  regardless of the choice made.
- Downloads list: right-click "Archive(s)" menu for extraction actions
  (Extract Now, Abort extraction, Auto Extract Enabled, Set Extraction
  Path, Set Archive Password, Validate Archive(s), Cleanup after
  Extraction), matching JDownloader's own context menu.
- Progress column now tracks archive-extraction progress (resetting to 0%
  and climbing as the archive decompresses) instead of staying frozen at
  100% from the finished download.

### Changed
- Scripts page: the "Run synchronously" and "Interval (ms)" fields are now
  only shown when the selected event trigger actually supports them
  (matching JDownloader), instead of being grayed out.
- Downloads list Size column shows a package's file count (e.g. "[3] 1.2
  GiB"), matching JDownloader.
- Downloads list Connection column no longer shows an icon for finished or
  disabled downloads, matching JDownloader (which leaves it blank there).
- Downloads list Speed column turns red while a global download speed
  limit is enabled, matching JDownloader.
- Toolbar Pause button (and its tray menu entry) now shows the same on/off
  checkbox indicator as the other toolbar toggles.

### Fixed
- An expanded package row in the Downloads/Link Grabber tree could still go
  visually stale in some cases (e.g. its only download finishing) even
  though it no longer collapsed on refresh — now always repaints.
- A headless JDownloader dialog that expired on its own (JDownloader
  abandons an unanswered dialog after a short delay while running
  headless) failed with an "Invalid ID" error instead of just being
  dismissed.
- Extension Manager: enable/disable now uses a proper checkbox column
  (locked and unchecked for extensions that aren't installed) followed by
  an Install/Remove button, matching JDownloader's own layout. Installing
  or removing an extension shows a modal indeterminate-progress dialog and
  restarts the local JDownloader process, as required for the change to
  take effect.
- Fixed a race condition where restarting the local JDownloader process
  (after installing/removing an extension, or on app close) could kill it
  before it finished flushing just-made changes (e.g. a newly added
  script) to disk, silently losing them.

## [0.2.0] - 2026-09-08

### Added
- Downloads and Link Grabber panels rebuilt on GTK4 `ColumnView`, with an
  expandable package/link tree, sortable/toggleable columns, and a
  properties panel for the current selection.
- Main toolbar fully wired to JDownloader: start/pause/stop downloads,
  move package top/up/down/bottom (acting on whichever tab is active),
  clipboard monitoring/auto-reconnect/premium/silent-mode toggles (with
  JDownloader-style on/off checkbox badges merged onto their icons),
  reconnect, and update-check — all reflecting live state via periodic
  status polling.
- Link Grabber: per-link variant selection (e.g. video resolution/format
  on YouTube and similar hosts), with background fetching of JDownloader's
  server-composited variant/status icons (opaque `kc.<hash>` keys with no
  static equivalent).
- Account manager UI and JDownloader favicon support for hoster icons.
- System tray icon and native/toast notifications (downloads complete,
  new links captured from the clipboard).
- Clipboard monitoring now mirrors JDownloader's own behavior: seeded from
  whatever's on the clipboard at startup (no spurious notification for
  pre-existing content), reset when monitoring is toggled off so
  re-enabling it re-evaluates the current clipboard, and notifies only
  once JDownloader confirms links were actually added (not silently
  dropped as duplicates).
- gDownloader-local settings store (`src/config.rs`) for preferences with
  no JDownloader config equivalent, such as per-package expand/collapse
  state.
- Full UI translation into 23 languages (Arabic, Czech, Danish, German,
  Greek, English, Spanish, Finnish, French, Hungarian, Indonesian, Italian,
  Japanese, Korean, Dutch, Norwegian, Polish, Portuguese, Russian, Swedish,
  Turkish, Ukrainian, Chinese), up from English/French only.
- Linux CI: automated build/test on every push and packaged `.deb` releases
  (amd64/arm64) attached to GitHub Releases on tag push.
- Published as a `.deb` package through a personal APT repository
  (`apt.serviam.cc`), updated automatically on every release.

### Fixed
- Link Grabber context menu's "Start Downloads" did nothing after the
  `ColumnView` migration (it was still unwrapping selection items as if
  the tree had no `TreeListRow` wrapper).
- Expanded packages in the Downloads and Link Grabber trees no longer
  collapse on every refresh — GTK's `TreeListModel` discards a row's
  expanded state whenever the underlying item is replaced, which happened
  on every tick a package's own fields changed (e.g. download progress,
  or a new link added to a linked package). Changed rows for an expanded
  package are now mutated in place instead of replaced.
- Download/package status icons no longer show as a broken/red icon while
  actively downloading (JDownloader hands out an opaque composited icon
  key for progress status, now fetched like variant icons).
- Link Grabber "Variant" column: dropdown arrow now uses JDownloader's
  actual combobox icon instead of a list-reorder icon, and variant
  thumbnail icons resolve correctly instead of failing to load.
- Delete/Suppr in the Downloads and Link Grabber lists no longer silently
  stops working a few seconds after the list is touched (the periodic
  refresh was tearing down the focused row without restoring keyboard
  focus).
- Deleting selected downloads/links now selects and focuses the row that
  slides into their place, instead of leaving nothing selected/focused.
- Download folder paths with accented characters (e.g. "Vidéos") were
  written to disk with `?` instead of the accented letter, because
  JDownloader's JVM was launched with an ASCII locale (`LANG=C`).
