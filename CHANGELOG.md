# Changelog

All notable changes to gDownloader are documented in this file.

## [Unreleased]

### Added
- A new "Add Links" dialog, matching JDownloader's own "Analyse and Add
  Links" window — opened from the File menu's "Analyse Text with Links"
  entry and the Link Collector's "+" button / right-click "Add New Links"
  entry. Lets you paste links/URLs/text, set a download destination (with a
  folder browser), package name, comment, extraction password, download
  password, priority, and Auto Extract, with a "Start Deep Link Analyse"
  option via a small arrow menu next to Continue. Pre-fills from
  JDownloader's own LinkgrabberSettings (last-used or default download
  folder, Auto Extract default, clipboard auto-fill on open) and
  auto-toggles "Information overwrites packagizer rules" when a custom
  field is set, mirroring JDownloader's own dialog behavior.
- Link Collector bottom bar: every button is now wired up, matching
  JDownloader's own (Clear, Delete disabled/offline/incomplete-archives,
  Confirm's Start/Add all/selected, and a Quick Settings menu for Add at
  top/Auto confirm/Auto start/Link filter plus Properties/Overview/Sidebar
  panel visibility toggles) — previously these were inert placeholders.
  "Add" also gained "Paste Links"/"Paste Links (Deep Analyse)", which submit
  the clipboard directly without opening the Add Links dialog, and the same
  "Add Links"/"Clear Downloadlist" buttons and menus (with JDownloader's own
  icons throughout) are now also in the Downloads list's bottom bar. Ctrl+L
  opens Add Links from anywhere, and Ctrl+V/Ctrl+Shift+V trigger Paste
  Links/Paste Links (Deep Analyse) while a links or downloads table has
  focus — the same shortcuts JDownloader uses, now also shown next to their
  menu entries (File menu, Link Collector right-click, both "Add" popovers)
  the way GTK shows native menu shortcuts. A few JDownloader actions
  ("Clear filtered links", "Add filtered stuff", "Add Container") stay
  disabled as they have no RemoteAPI equivalent reachable from a remote
  client, or aren't implemented yet.
- Downloads list bottom bar: the "Filter" field is now a category combobox
  (File Name/File Path/Hoster/Package Name/Comment/Comment(Package)/Status,
  each with JDownloader's own icon) linked to a text field whose
  placeholder changes with the selected category, and the "All Downloads"
  quick filter dropdown now shows JDownloader's own labels and icons for
  each view (Running/Failed/File exists/Offline/Skipped/Successful/
  Pending), matching `DownloadsTableSearchField`/`View`. Its Quick Settings
  menu now has working spinners for Max. chunks per download/Max.
  simultaneous downloads/Max. sim. downloads per hoster/Speed limit
  (matching JDownloader's own editors) plus Properties/Overview panel
  visibility toggles, in place of the previous inert placeholder rows.
- All bottom-bar arrow menu buttons (Add/Delete/Confirm/Quick Settings, in
  both the Link Collector and Downloads list) now point down, matching the
  usual GTK dropdown/combobox convention.
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
- Settings changed via the Downloads list's Quick Settings menu or
  Settings > General (Max. chunks/simultaneous downloads/per hoster, speed
  limit) could silently fail to survive an app restart: JDownloader running
  headless buffers its own config writes to disk and only flushes them when
  download or Link Collector activity happens to reschedule that flush,
  which a plain settings change never does. gDownloader now disables that
  buffering on startup, so every setting change is written to disk
  immediately, matching JDownloader's own desktop (non-headless) behavior.
- Those same settings could show a wrong, generic default value (e.g. "3"
  simultaneous downloads) right after startup instead of the real saved
  one, because they were read before JDownloader's RemoteAPI had actually
  finished starting up.
- A setting changed right before closing gDownloader (via the window's
  close button, which waits for JDownloader to shut down gracefully so it
  can flush its own delayed config writes) could still be lost: the change
  itself is saved on a detached background thread that isn't guaranteed to
  have even sent its request yet by the time JDownloader is asked to exit.
  Closing the window now waits (briefly) for every such pending write to
  actually complete first.
- "Max. chunks per download"/"Max. simultaneous downloads"/"Max. sim.
  Downloads per Hoster" were shown in two places (Settings > General, and
  the Downloads list's own Quick Settings menu), each independently
  polling JDownloader — changing one saved correctly, but the other kept
  showing whatever value it had loaded at startup, even after being
  reopened. Both now read and write a single shared, cached state, and
  each re-syncs itself with it whenever shown (opening the Quick Settings
  popover, or the Settings tab) — free once loaded, since JDownloader
  never changes these on its own, so no repeated network polling either.
- Link Collector context menu: "Start Downloads" on a selected package did
  nothing (it silently skipped package rows instead of expanding them to
  their child links, so it only ever worked on individually selected
  files). "Start All Downloads" also did nothing when the download list was
  still empty, because it resumed the existing download queue instead of
  pushing the Link Collector's own links to it.
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
