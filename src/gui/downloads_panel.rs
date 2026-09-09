use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;
use serde_json::Value;

use crate::gui::package_tree::{
    build_name_column, sync_store, sync_store_keep_expanded, tree_item, PackageTree,
};
use crate::jd::{GeneralSettings, GraphicalUserInterfaceSettings, JdApi};

#[derive(Clone, Debug)]
pub struct DownloadRow {
    pub uuid: String,
    pub package_uuid: i64,
    pub package_name: String,
    pub name: String,
    pub url: String,
    pub priority: String,
    pub file_type_icon: gio::Icon,
    pub host_icon: gio::Icon,
    pub host_name: String,
    /// True when a real PNG was resolved for this host (false = using fallback icon).
    /// Included in PartialEq so the store rebuilds when a favicon download completes.
    pub has_host_icon: bool,
    pub bytes_total: i64,
    pub bytes_loaded: i64,
    pub size_text: String,
    pub status: String,
    pub status_icon: gio::Icon,
    /// True when the status icon key actually resolved to a real image.
    /// JDownloader hands out an opaque `"kc.<hash>"` key (a server-composited
    /// progress icon) while a link is actively downloading, so this needs
    /// fetching before it resolves — included in `PartialEq` so the row
    /// rebinds once that fetch completes, even if `status` text itself
    /// didn't change meanwhile.
    pub has_status_icon: bool,
    pub speed_text: String,
    pub eta_text: String,
    pub loaded_text: String,
    pub save_to: String,
    pub comment: String,
    pub skipped: bool,
    pub running: bool,
    pub finished: bool,
    pub enabled: bool,
    pub show_progress: bool,
    /// `(current, total)` bytes from `advancedStatus.PluginProgress` when
    /// its `id` is `"EXTRACTION"` — JDownloader reuses `DownloadLink`'s
    /// progress bar for the archive-extraction phase too
    /// (`ProgressColumn`/`ExtractionProgress`, verified against the SVN),
    /// with `current`/`total` counting decompressed bytes rather than
    /// downloaded ones. `None` outside that phase, in which case the
    /// Progress column falls back to `bytes_loaded`/`bytes_total` as usual.
    pub extraction_progress: Option<(i64, i64)>,
}

impl PartialEq for DownloadRow {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
            && self.package_uuid == other.package_uuid
            && self.package_name == other.package_name
            && self.name == other.name
            && self.url == other.url
            && self.priority == other.priority
            && self.host_name == other.host_name
            && self.has_host_icon == other.has_host_icon
            && self.bytes_total == other.bytes_total
            && self.bytes_loaded == other.bytes_loaded
            && self.size_text == other.size_text
            && self.status == other.status
            && self.has_status_icon == other.has_status_icon
            && self.speed_text == other.speed_text
            && self.eta_text == other.eta_text
            && self.loaded_text == other.loaded_text
            && self.save_to == other.save_to
            && self.comment == other.comment
            && self.skipped == other.skipped
            && self.running == other.running
            && self.finished == other.finished
            && self.enabled == other.enabled
            && self.show_progress == other.show_progress
            && self.extraction_progress == other.extraction_progress
    }
}

/// A package (group) row: the collapsible parent of a set of [`DownloadRow`]s
/// in the downloads tree, mirroring JDownloader's package rows.
#[derive(Clone, Debug)]
pub struct PackageRow {
    pub uuid: i64,
    pub name: String,
    pub save_to: String,
    pub child_count: i64,
    pub bytes_total: i64,
    pub bytes_loaded: i64,
    pub size_text: String,
    pub loaded_text: String,
    pub status: String,
    pub status_icon: gio::Icon,
    /// True when the status icon key actually resolved to a real image (see
    /// `DownloadRow::has_status_icon`).
    pub has_status_icon: bool,
    pub speed_text: String,
    pub eta_text: String,
    pub enabled: bool,
    pub finished: bool,
    pub running: bool,
    pub comment: String,
    /// Sum of `(current, total)` across the package's children currently
    /// mid-extraction (see [`DownloadRow::extraction_progress`]) — JD has no
    /// package-level equivalent over the API, so this is our own
    /// aggregation, computed in `JdApi::query_downloads`. `None` when no
    /// child is extracting, in which case the Progress column falls back to
    /// `bytes_loaded`/`bytes_total` as usual.
    pub extraction_progress: Option<(i64, i64)>,
}

impl PartialEq for PackageRow {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
            && self.name == other.name
            && self.save_to == other.save_to
            && self.child_count == other.child_count
            && self.bytes_total == other.bytes_total
            && self.bytes_loaded == other.bytes_loaded
            && self.size_text == other.size_text
            && self.loaded_text == other.loaded_text
            && self.status == other.status
            && self.has_status_icon == other.has_status_icon
            && self.speed_text == other.speed_text
            && self.eta_text == other.eta_text
            && self.enabled == other.enabled
            && self.finished == other.finished
            && self.running == other.running
            && self.comment == other.comment
            && self.extraction_progress == other.extraction_progress
    }
}

pub struct DownloadsPanel {
    pub widget: gtk4::Box,
    /// Top-level store of packages. Each package's children live in their
    /// own store, held in `child_stores`, lazily attached by the
    /// `TreeListModel`'s create function.
    pub store: gio::ListStore,
    pub child_stores: Rc<RefCell<HashMap<i64, gio::ListStore>>>,
    pub selection: gtk4::MultiSelection,
    pub view: gtk4::ColumnView,
    pub overview: Rc<crate::gui::overview_panel::OverviewPanel>,
    /// Shared with `start_refresh`, which consults it to avoid ever
    /// splicing an *expanded* package's own row back into `store` (that
    /// would discard its `TreeListRow`'s expanded state — see
    /// `sync_store_keep_expanded`).
    expanded_state: Rc<RefCell<HashMap<i64, bool>>>,
    /// One [`PackageLiveRefresh`] registry per column that shows package
    /// data, collected while building `view`'s columns. `start_refresh`
    /// fans a mutated-in-place package out through all of them so an
    /// expanded package's row keeps repainting on every tick.
    package_live: Vec<PackageLiveRefresh>,
    /// Whether JDownloader's global download speed cap is on — read by the
    /// Speed column to color its text red, mirroring `SpeedColumn`. Updated
    /// by `start_refresh`'s poll.
    speed_limited: Rc<Cell<bool>>,
}

/// Keys identifying each Overview stat, passed to
/// `overview_panel::OverviewPanel::set_value`.
const OVERVIEW_PACKAGES: &str = "packages";
const OVERVIEW_LINKS: &str = "links";
const OVERVIEW_SIZE: &str = "size";
const OVERVIEW_SPEED: &str = "speed";
const OVERVIEW_LOADED: &str = "loaded";
const OVERVIEW_REMAINING: &str = "remaining";
const OVERVIEW_ETA: &str = "eta";
const OVERVIEW_RUNNING: &str = "running";
const OVERVIEW_FINISHED: &str = "finished";
const OVERVIEW_SKIPPED: &str = "skipped";
const OVERVIEW_FAILED: &str = "failed";

/// Overview stats and their JDownloader `GraphicalUserInterfaceSettings`
/// visibility flags (JDownloader's own key names, reused verbatim), in
/// `DownloadOverview.createDataEntries()`'s declaration order — that order
/// drives the 2-row grid packing (see `overview_panel::relayout`). Defaults
/// mirror JDownloader's `@DefaultBooleanValue` on each `is...Visible()`.
fn overview_field_defs() -> Vec<crate::gui::overview_panel::OverviewFieldDef> {
    use crate::gui::overview_panel::OverviewFieldDef;
    vec![
        OverviewFieldDef::new(
            OVERVIEW_PACKAGES,
            "OverviewPanelDownloadPackageCountVisible",
            tr!("Packages").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_LINKS,
            "OverviewPanelDownloadLinkCountVisible",
            tr!("Links").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_SIZE,
            "OverviewPanelDownloadTotalBytesVisible",
            tr!("Size").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_SPEED,
            "OverviewPanelDownloadSpeedVisible",
            tr!("Speed").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_LOADED,
            "OverviewPanelDownloadBytesLoadedVisible",
            tr!("Loaded").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_REMAINING,
            "OverviewPanelDownloadBytesRemainingVisible",
            tr!("Remaining").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_ETA,
            "OverviewPanelDownloadETAVisible",
            tr!("ETA").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_RUNNING,
            "OverviewPanelDownloadRunningDownloadsCountVisible",
            tr!("Running downloads").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_FINISHED,
            "OverviewPanelDownloadLinksFinishedCountVisible",
            tr!("Finished downloads").to_string(),
            false,
        ),
        OverviewFieldDef::new(
            OVERVIEW_SKIPPED,
            "OverviewPanelDownloadLinksSkippedCountVisible",
            tr!("Skipped downloads").to_string(),
            false,
        ),
        OverviewFieldDef::new(
            OVERVIEW_FAILED,
            "OverviewPanelDownloadLinksFailedCountVisible",
            tr!("Failed downloads").to_string(),
            false,
        ),
    ]
}

/// Maps an overview stat key to its JDownloader config key.
fn overview_cfg_key(field: &str) -> Option<&'static str> {
    overview_field_defs().into_iter().find(|d| d.key == field).map(|d| d.cfg_key)
}

fn overview_actions(gui_settings: GraphicalUserInterfaceSettings) -> crate::gui::overview_panel::OverviewActions {
    crate::gui::overview_panel::OverviewActions {
        set_field_visible: Box::new(move |field, visible| {
            let Some(cfg_key) = overview_cfg_key(field) else {
                return;
            };
            let gui_settings = gui_settings.clone();
            crate::gui::spawn::api_fire(move || {
                let _ = gui_settings.set_flag(cfg_key, visible);
            });
        }),
    }
}

/// `GraphicalUserInterfaceSettings` config keys backing this panel's "visible
/// fields" checkboxes (the priority dropdown has no key of its own: it's
/// shown/hidden together with the comment field, as JDownloader itself does).
const CFG_PACKAGE_NAME_VISIBLE: &str = "DownloadsPropertiesPanelPackagenameVisible";
const CFG_FILE_NAME_VISIBLE: &str = "DownloadsPropertiesPanelFilenameVisible";
const CFG_SAVE_TO_VISIBLE: &str = "DownloadsPropertiesPanelSaveToVisible";
const CFG_SOURCE_VISIBLE: &str = "DownloadsPropertiesPanelDownloadFromVisible";
const CFG_COMMENT_VISIBLE: &str = "DownloadsPropertiesPanelCommentVisible";

/// Maps a properties-panel field key to its JDownloader config key.
fn field_cfg_key(field: &str) -> Option<&'static str> {
    use crate::gui::properties_panel::*;
    match field {
        FIELD_PACKAGE_NAME => Some(CFG_PACKAGE_NAME_VISIBLE),
        FIELD_FILE_NAME => Some(CFG_FILE_NAME_VISIBLE),
        FIELD_SAVE_TO => Some(CFG_SAVE_TO_VISIBLE),
        FIELD_SOURCE => Some(CFG_SOURCE_VISIBLE),
        FIELD_COMMENT => Some(CFG_COMMENT_VISIBLE),
        _ => None,
    }
}

fn properties_actions(
    api: Arc<JdApi>,
    gui_settings: GraphicalUserInterfaceSettings,
) -> crate::gui::properties_panel::PropertiesActions {
    crate::gui::properties_panel::PropertiesActions {
        rename_link: Box::new({
            let api = api.clone();
            move |id, name| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.rename_download_link(id, &name);
                });
            }
        }),
        rename_package: Box::new({
            let api = api.clone();
            move |id, name| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.rename_download_package(id, &name);
                });
            }
        }),
        set_comment: Box::new({
            let api = api.clone();
            move |link_id, comment| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.set_download_comment(&[link_id], &[], false, &comment);
                });
            }
        }),
        set_priority: Box::new({
            let api = api.clone();
            move |link_id, priority| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.set_download_priority(&[link_id], &[], &priority);
                });
            }
        }),
        set_download_directory: Box::new({
            let api = api.clone();
            move |package_id, dir| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.set_download_directory(&dir, &[package_id]);
                });
            }
        }),
        set_field_visible: Box::new(move |field, visible| {
            let Some(cfg_key) = field_cfg_key(field) else {
                return;
            };
            let gui_settings = gui_settings.clone();
            crate::gui::spawn::api_fire(move || {
                let _ = gui_settings.set_flag(cfg_key, visible);
            });
        }),
    }
}

/// This panel's package row type, bound to [`crate::gui::package_tree::PackageLiveRefresh`].
type PackageLiveRefresh = crate::gui::package_tree::PackageLiveRefresh<PackageRow>;

/// 0.0-1.0 download fraction for a package row, shared between the Progress
/// column's initial bind and its `PackageLiveRefresh` repaint closure.
fn package_progress_fraction(pkg: &PackageRow) -> f64 {
    if let Some((current, total)) = pkg.extraction_progress {
        if total > 0 {
            return current as f64 / total as f64;
        }
    }
    if pkg.bytes_total > 0 {
        pkg.bytes_loaded as f64 / pkg.bytes_total as f64
    } else if pkg.finished {
        1.0
    } else {
        0.0
    }
}

impl DownloadsPanel {
    pub fn build(api: Arc<JdApi>, gui_settings: GraphicalUserInterfaceSettings) -> Self {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        page.set_vexpand(true);
        page.set_hexpand(true);

        let PackageTree {
            store,
            child_stores,
            expanded_state,
            selection,
            ..
        } = PackageTree::build::<PackageRow>("downloads", |p| p.uuid);

        let view = gtk4::ColumnView::new(Some(selection.clone()));
        view.set_vexpand(true);
        view.set_hexpand(true);
        view.set_show_row_separators(true);
        view.set_show_column_separators(false);

        // Text column showing `pkg_accessor(&PackageRow)` for package rows
        // (depth 0) and `link_accessor(&DownloadRow)` for link rows (depth 1).
        let dual_text_col = |pkg_accessor: fn(&PackageRow) -> &str,
                             link_accessor: fn(&DownloadRow) -> &str,
                             xalign: f32,
                             min_width: i32,
                             title: &str,
                             resizable: bool,
                             expand: bool,
                             ellipsize: bool,
                             red_when_limited: Option<Rc<Cell<bool>>>|
         -> (gtk4::ColumnViewColumn, PackageLiveRefresh) {
            let live: PackageLiveRefresh = Rc::new(RefCell::new(HashMap::new()));
            // Only the Speed column passes `Some` here — mirrors
            // `SpeedColumn.configureRendererComponent`, which colors the
            // whole column red whenever a global download speed cap is on,
            // regardless of any row's actual speed value.
            let set_text = move |label: &gtk4::Label, text: &str| {
                if red_when_limited.as_ref().is_some_and(|f| f.get()) {
                    label.set_markup(&format!(
                        "<span foreground=\"red\">{}</span>",
                        glib::markup_escape_text(text)
                    ));
                } else {
                    label.set_text(text);
                }
            };
            let factory = gtk4::SignalListItemFactory::new();
            factory.connect_setup(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let label = gtk4::Label::new(None);
                label.set_halign(if xalign == 1.0 {
                    gtk4::Align::End
                } else {
                    gtk4::Align::Start
                });
                label.set_xalign(xalign);
                if ellipsize {
                    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    label.set_has_tooltip(true);
                }
                list_item.set_child(Some(&label));
            });
            factory.connect_bind({
                let live = live.clone();
                let set_text = set_text.clone();
                move |_, list_item| {
                    let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                    let Some((tree_row, obj)) = tree_item(list_item) else {
                        return;
                    };
                    let Some(label) = list_item.child().and_downcast::<gtk4::Label>() else {
                        return;
                    };
                    if tree_row.depth() == 0 {
                        let pkg = obj.borrow::<PackageRow>();
                        set_text(&label, pkg_accessor(&pkg));
                        if ellipsize {
                            label.set_tooltip_text(Some(pkg_accessor(&pkg)));
                        }
                        let uuid = pkg.uuid;
                        let label = label.clone();
                        let set_text = set_text.clone();
                        live.borrow_mut().insert(
                            uuid,
                            Box::new(move |pkg: &PackageRow| {
                                let text = pkg_accessor(pkg);
                                set_text(&label, text);
                                if ellipsize {
                                    label.set_tooltip_text(Some(text));
                                }
                            }),
                        );
                    } else {
                        let row = obj.borrow::<DownloadRow>();
                        let text = link_accessor(&row);
                        set_text(&label, text);
                        if ellipsize {
                            label.set_tooltip_text(Some(text));
                        }
                    }
                }
            });
            factory.connect_unbind({
                let live = live.clone();
                move |_, list_item| {
                    let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                    if let Some((_, obj)) = tree_item(list_item) {
                        // Not `tree_row.depth() == 0`: a row's depth can no
                        // longer be trusted once it's being torn down (e.g.
                        // as part of removing rows), so the underlying
                        // `BoxedAnyObject`'s actual type is checked directly
                        // instead — `borrow` would panic on a mismatch.
                        if let Ok(pkg) = obj.try_borrow::<PackageRow>() {
                            live.borrow_mut().remove(&pkg.uuid);
                        }
                    }
                }
            });
            let col = gtk4::ColumnViewColumn::new(Some(title), Some(factory));
            col.set_fixed_width(min_width);
            col.set_resizable(resizable);
            col.set_expand(expand);
            (col, live)
        };
        let mut package_live: Vec<PackageLiveRefresh> = Vec::new();
        // Set by `start_refresh`'s poll; only the Speed column reads it.
        let speed_limited: Rc<Cell<bool>> = Rc::new(Cell::new(false));

        // Icon column: only meaningful for link rows (e.g. the hoster favicon);
        // package rows show nothing, matching JDownloader.
        let icon_col = |accessor: fn(&DownloadRow) -> &gio::Icon,
                        tooltip: fn(&DownloadRow) -> &str,
                        min_width: i32,
                        title: &str|
         -> gtk4::ColumnViewColumn {
            let factory = gtk4::SignalListItemFactory::new();
            factory.connect_setup(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let image = gtk4::Image::new();
                image.set_halign(gtk4::Align::Start);
                image.set_pixel_size(16);
                image.set_has_tooltip(true);
                let list_item_obj = list_item.clone();
                image.connect_query_tooltip(move |_, _x, _y, _kb, ttip| {
                    let list_item = list_item_obj.downcast_ref::<gtk4::ListItem>().unwrap();
                    let Some((tree_row, obj)) = tree_item(list_item) else {
                        return false;
                    };
                    if tree_row.depth() == 0 {
                        return false;
                    }
                    let row = obj.borrow::<DownloadRow>();
                    ttip.set_text(Some(tooltip(&row)));
                    true
                });
                list_item.set_child(Some(&image));
            });
            factory.connect_bind(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let Some((tree_row, obj)) = tree_item(list_item) else {
                    return;
                };
                let Some(image) = list_item.child().and_downcast::<gtk4::Image>() else {
                    return;
                };
                if tree_row.depth() == 0 {
                    image.set_visible(false);
                } else {
                    let row = obj.borrow::<DownloadRow>();
                    image.set_from_gicon(accessor(&row));
                    image.set_visible(true);
                }
            });
            let col = gtk4::ColumnViewColumn::new(Some(title), Some(factory));
            col.set_fixed_width(min_width);
            col.set_resizable(false);
            col
        };

        // Columns the "choose visible columns" header menu can toggle (the
        // Name column itself always stays visible, matching JDownloader).
        let mut togglable_columns: Vec<(&'static str, String, gtk4::ColumnViewColumn)> = Vec::new();

        // Name: TreeExpander (package/link indentation + expand triangle) +
        // file/package icon + label.
        {
            let col = build_name_column::<PackageRow, DownloadRow>(
                tr!("Name").as_ref(),
                220,
                "downloads",
                expanded_state.clone(),
                |p| p.uuid,
                |p| &p.name,
                (|r| &r.file_type_icon, |r| &r.name),
            );
            view.append_column(&col);
        }

        // Progress: see `gui::cells::progress_cell` for how the bar/label
        // overlay is built; only the fraction computation is specific to
        // download rows here.
        {
            let progress_live: PackageLiveRefresh = Rc::new(RefCell::new(HashMap::new()));
            let col = crate::gui::cells::progress_cell::build_column(
                tr!("Progress").as_ref(),
                165,
                false,
                |list_item| {
                    let (tree_row, obj) = tree_item(list_item)?;
                    let fraction = if tree_row.depth() == 0 {
                        package_progress_fraction(&obj.borrow::<PackageRow>())
                    } else {
                        let row = obj.borrow::<DownloadRow>();
                        if let Some((current, total)) = row.extraction_progress {
                            if total > 0 {
                                current as f64 / total as f64
                            } else {
                                0.0
                            }
                        } else if row.bytes_total > 0 {
                            row.bytes_loaded as f64 / row.bytes_total as f64
                        } else if row.finished {
                            1.0
                        } else {
                            0.0
                        }
                    };
                    Some(fraction)
                },
                {
                    let progress_live = progress_live.clone();
                    move |list_item, bar, label| {
                        let Some((tree_row, obj)) = tree_item(list_item) else {
                            return;
                        };
                        if tree_row.depth() != 0 {
                            return;
                        }
                        let uuid = obj.borrow::<PackageRow>().uuid;
                        let bar = bar.clone();
                        let label = label.clone();
                        progress_live.borrow_mut().insert(
                            uuid,
                            Box::new(move |pkg: &PackageRow| {
                                let fraction = package_progress_fraction(pkg);
                                bar.set_fraction(fraction);
                                label.set_text(&format!("{:.1}%", fraction * 100.0));
                            }),
                        );
                    }
                },
                {
                    let progress_live = progress_live.clone();
                    move |list_item| {
                        if let Some((_, obj)) = tree_item(list_item) {
                            // See `dual_text_col`'s `connect_unbind` for why
                            // this checks the object's actual type instead
                            // of `tree_row.depth()`.
                            if let Ok(pkg) = obj.try_borrow::<PackageRow>() {
                                progress_live.borrow_mut().remove(&pkg.uuid);
                            }
                        }
                    }
                },
            );
            togglable_columns.push(("progress", tr!("Progress").to_string(), col.clone()));
            view.append_column(&col);
            package_live.push(progress_live);
        }

        // Size: a package row shows the child count in brackets next to the
        // total size, mirroring `SizeColumn`'s two-`RenderLabel` layout
        // (`countRenderer` left-aligned, `sizeRenderer` right-aligned in the
        // same cell) — `isFileCountInSizeColumnVisible` gates this in JD,
        // defaulting (and always, here) to on. A link row has no count of
        // its own, matching JD leaving `countRenderer` blank there.
        {
            let size_live: PackageLiveRefresh = Rc::new(RefCell::new(HashMap::new()));
            let factory = gtk4::SignalListItemFactory::new();
            factory.connect_setup(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                hbox.set_hexpand(true);
                let count_label = gtk4::Label::new(None);
                count_label.set_halign(gtk4::Align::Start);
                let size_label = gtk4::Label::new(None);
                size_label.set_halign(gtk4::Align::End);
                size_label.set_hexpand(true);
                hbox.append(&count_label);
                hbox.append(&size_label);
                list_item.set_child(Some(&hbox));
            });
            factory.connect_bind({
                let size_live = size_live.clone();
                move |_, list_item| {
                    let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                    let Some((tree_row, obj)) = tree_item(list_item) else {
                        return;
                    };
                    let Some(hbox) = list_item.child().and_downcast::<gtk4::Box>() else {
                        return;
                    };
                    let Some(count_label) = hbox.first_child().and_downcast::<gtk4::Label>()
                    else {
                        return;
                    };
                    let Some(size_label) = count_label.next_sibling().and_downcast::<gtk4::Label>()
                    else {
                        return;
                    };
                    if tree_row.depth() == 0 {
                        let pkg = obj.borrow::<PackageRow>();
                        count_label.set_text(&format!("[{}]", pkg.child_count));
                        size_label.set_text(&pkg.size_text);
                        let uuid = pkg.uuid;
                        let count_label = count_label.clone();
                        let size_label = size_label.clone();
                        size_live.borrow_mut().insert(
                            uuid,
                            Box::new(move |pkg: &PackageRow| {
                                count_label.set_text(&format!("[{}]", pkg.child_count));
                                size_label.set_text(&pkg.size_text);
                            }),
                        );
                    } else {
                        let row = obj.borrow::<DownloadRow>();
                        count_label.set_text("");
                        size_label.set_text(&row.size_text);
                    }
                }
            });
            factory.connect_unbind({
                let size_live = size_live.clone();
                move |_, list_item| {
                    let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                    if let Some((_, obj)) = tree_item(list_item) {
                        // See `dual_text_col`'s `connect_unbind` for why
                        // this checks the object's actual type instead of
                        // `tree_row.depth()`.
                        if let Ok(pkg) = obj.try_borrow::<PackageRow>() {
                            size_live.borrow_mut().remove(&pkg.uuid);
                        }
                    }
                }
            });
            let size_col = gtk4::ColumnViewColumn::new(Some(tr!("Size").as_ref()), Some(factory));
            size_col.set_fixed_width(110);
            size_col.set_resizable(false);
            togglable_columns.push(("size", tr!("Size").to_string(), size_col.clone()));
            view.append_column(&size_col);
            package_live.push(size_live);
        }

        let hoster_col = icon_col(
            |r| &r.host_icon,
            |r| &r.host_name,
            90,
            tr!("Hoster").as_ref(),
        );
        togglable_columns.push(("hoster", tr!("Hoster").to_string(), hoster_col.clone()));
        view.append_column(&hoster_col);
        // Connection: link-only (JDownloader also keeps this blank on package
        // rows). Mirrors `ConnectionColumn.configureRendererComponent`: the
        // cell is blank by default (`resetRenderer`), and each icon is a
        // narrow, specific condition rather than a general link-state
        // summary — notably there's *no* icon for finished or disabled;
        // those show nothing here (a finished link's checkmark lives in the
        // Status column instead, via `FinalLinkState`'s own icon).
        {
            let factory = gtk4::SignalListItemFactory::new();
            factory.connect_setup(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
                row_box.set_halign(gtk4::Align::Start);
                let icons: [&'static str; 2] = [
                    crate::gui::icon_key::ICON_SKIPPED,
                    crate::gui::icon_key::ICON_MEDIA_PLAYBACK_START,
                ];
                for icon in icons {
                    let img = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(icon));
                    img.set_pixel_size(14);
                    img.set_visible(false);
                    row_box.append(&img);
                }
                list_item.set_child(Some(&row_box));
            });
            factory.connect_bind(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let Some((tree_row, obj)) = tree_item(list_item) else {
                    return;
                };
                let Some(row_box) = list_item.child().and_downcast::<gtk4::Box>() else {
                    return;
                };
                let visibilities = if tree_row.depth() == 0 {
                    [false, false]
                } else {
                    let row = obj.borrow::<DownloadRow>();
                    [row.skipped, row.running && !row.finished]
                };
                let mut child = row_box.first_child();
                for visible in visibilities {
                    if let Some(ref c) = child {
                        if let Some(img) = c.downcast_ref::<gtk4::Image>() {
                            img.set_visible(visible);
                        }
                        child = c.next_sibling();
                    }
                }
            });
            let col = gtk4::ColumnViewColumn::new(Some(tr!("Connection").as_ref()), Some(factory));
            col.set_fixed_width(80);
            col.set_resizable(false);
            togglable_columns.push(("connection", tr!("Connection").to_string(), col.clone()));
            view.append_column(&col);
        }

        // Status: icon + text, mirroring JDownloader's TaskColumn (the
        // `status`/`statusIconKey` fields returned by the API are already
        // the exact rich label/icon JD's own column would show, for both
        // links and packages).
        {
            let status_live: PackageLiveRefresh = Rc::new(RefCell::new(HashMap::new()));
            let factory = gtk4::SignalListItemFactory::new();
            factory.connect_setup(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
                hbox.set_halign(gtk4::Align::Start);
                let icon = gtk4::Image::new();
                icon.set_pixel_size(14);
                icon.set_valign(gtk4::Align::Center);
                let label = gtk4::Label::new(None);
                label.set_xalign(0.0);
                label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                label.set_has_tooltip(true);
                hbox.append(&icon);
                hbox.append(&label);
                list_item.set_child(Some(&hbox));
            });
            factory.connect_bind({
                let status_live = status_live.clone();
                move |_, list_item| {
                    let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                    let Some((tree_row, obj)) = tree_item(list_item) else {
                        return;
                    };
                    let Some(hbox) = list_item.child().and_downcast::<gtk4::Box>() else {
                        return;
                    };
                    let Some(icon) = hbox.first_child().and_downcast::<gtk4::Image>() else {
                        return;
                    };
                    let Some(label) = icon.next_sibling().and_downcast::<gtk4::Label>() else {
                        return;
                    };
                    let (status, status_icon) = if tree_row.depth() == 0 {
                        let pkg = obj.borrow::<PackageRow>();
                        (pkg.status.clone(), pkg.status_icon.clone())
                    } else {
                        let row = obj.borrow::<DownloadRow>();
                        (row.status.clone(), row.status_icon.clone())
                    };
                    icon.set_from_gicon(&status_icon);
                    icon.set_visible(!status.is_empty());
                    label.set_text(&status);
                    label.set_tooltip_text(Some(&status));
                    if tree_row.depth() == 0 {
                        let uuid = obj.borrow::<PackageRow>().uuid;
                        let icon = icon.clone();
                        let label = label.clone();
                        status_live.borrow_mut().insert(
                            uuid,
                            Box::new(move |pkg: &PackageRow| {
                                icon.set_from_gicon(&pkg.status_icon);
                                icon.set_visible(!pkg.status.is_empty());
                                label.set_text(&pkg.status);
                                label.set_tooltip_text(Some(&pkg.status));
                            }),
                        );
                    }
                }
            });
            factory.connect_unbind({
                let status_live = status_live.clone();
                move |_, list_item| {
                    let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                    if let Some((_, obj)) = tree_item(list_item) {
                        // See `dual_text_col`'s `connect_unbind` for why this
                        // checks the object's actual type instead of
                        // `tree_row.depth()`.
                        if let Ok(pkg) = obj.try_borrow::<PackageRow>() {
                            status_live.borrow_mut().remove(&pkg.uuid);
                        }
                    }
                }
            });
            let col = gtk4::ColumnViewColumn::new(Some(tr!("Status").as_ref()), Some(factory));
            col.set_fixed_width(110);
            col.set_resizable(false);
            togglable_columns.push(("status", tr!("Status").to_string(), col.clone()));
            view.append_column(&col);
            package_live.push(status_live);
        }
        let (speed_col, speed_live) = dual_text_col(
            |p| &p.speed_text,
            |r| &r.speed_text,
            1.0,
            90,
            tr!("Speed").as_ref(),
            false,
            false,
            false,
            Some(speed_limited.clone()),
        );
        togglable_columns.push(("speed", tr!("Speed").to_string(), speed_col.clone()));
        view.append_column(&speed_col);
        package_live.push(speed_live);

        let (eta_col, eta_live) = dual_text_col(
            |p| &p.eta_text,
            |r| &r.eta_text,
            1.0,
            90,
            tr!("ETA").as_ref(),
            false,
            false,
            false,
            None,
        );
        togglable_columns.push(("eta", tr!("ETA").to_string(), eta_col.clone()));
        view.append_column(&eta_col);
        package_live.push(eta_live);

        let (loaded_col, loaded_live) = dual_text_col(
            |p| &p.loaded_text,
            |r| &r.loaded_text,
            1.0,
            90,
            tr!("Loaded").as_ref(),
            false,
            false,
            false,
            None,
        );
        togglable_columns.push(("loaded", tr!("Loaded").to_string(), loaded_col.clone()));
        view.append_column(&loaded_col);
        package_live.push(loaded_live);

        let (save_to_col, save_to_live) = dual_text_col(
            |p| &p.save_to,
            |r| &r.save_to,
            0.0,
            220,
            tr!("Save To").as_ref(),
            false,
            true,
            true,
            None,
        );
        togglable_columns.push(("save_to", tr!("Save To").to_string(), save_to_col.clone()));
        view.append_column(&save_to_col);
        package_live.push(save_to_live);

        let (comment_col, comment_live) = dual_text_col(
            |p| &p.comment,
            |r| &r.comment,
            0.0,
            220,
            tr!("Comment").as_ref(),
            false,
            false,
            true,
            None,
        );
        togglable_columns.push(("comment", tr!("Comment").to_string(), comment_col.clone()));
        view.append_column(&comment_col);
        package_live.push(comment_live);

        let table_scroll = gtk4::ScrolledWindow::builder()
            .child(&view)
            .vexpand(true)
            .hexpand(true)
            .build();

        // "Choose visible columns" button, overlaid at the top-right corner
        // of the table (GTK's `ColumnViewColumn::header-menu` turned out to
        // show no visible affordance in this GTK version, so a real button
        // is used instead, styled like the properties panel's own "visible
        // fields" button). Visibility is persisted in gDownloader's own
        // config: JDownloader keeps this as internal Swing table-model
        // state with no RemoteAPI exposure.
        let columns_btn = gtk4::MenuButton::new();
        let columns_btn_icon = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
            crate::gui::icon_key::ICON_EXTTABLE_COLUMNBUTTON,
        ));
        columns_btn_icon.set_pixel_size(10);
        columns_btn.set_child(Some(&columns_btn_icon));
        columns_btn.set_has_frame(false);
        columns_btn.set_tooltip_text(Some(tr!("Choose visible columns").as_ref()));
        columns_btn.set_halign(gtk4::Align::End);
        columns_btn.set_valign(gtk4::Align::Start);
        columns_btn.set_margin_top(2);
        columns_btn.set_margin_end(2);
        {
            let saved = crate::config::get_bool_map("downloads_columns");
            let columns_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            columns_box.set_margin_start(8);
            columns_box.set_margin_end(8);
            columns_box.set_margin_top(8);
            columns_box.set_margin_bottom(8);
            for (key, title, col) in &togglable_columns {
                let visible = saved.get(*key).copied().unwrap_or(true);
                col.set_visible(visible);
                let check = gtk4::CheckButton::with_label(title);
                check.set_active(visible);
                check.connect_toggled({
                    let col = col.clone();
                    let key = key.to_string();
                    move |c| {
                        col.set_visible(c.is_active());
                        crate::config::set_bool_entry("downloads_columns", &key, c.is_active());
                    }
                });
                columns_box.append(&check);
            }
            let popover = gtk4::Popover::new();
            popover.set_child(Some(&columns_box));
            columns_btn.set_popover(Some(&popover));
        }

        let table_overlay = gtk4::Overlay::new();
        table_overlay.set_child(Some(&table_scroll));
        table_overlay.add_overlay(&columns_btn);
        table_overlay.set_vexpand(true);
        table_overlay.set_hexpand(true);

        page.append(&table_overlay);

        // Properties panel: shown for the selected download, hidden otherwise.
        let properties = Rc::new(crate::gui::properties_panel::PropertiesPanel::build(
            properties_actions(api, gui_settings.clone()),
        ));
        page.append(&properties.widget);
        crate::gui::properties_panel::restore_field_visibility(properties.clone(), {
            let gui_settings = gui_settings.clone();
            move || gui_settings.is_ready()
        }, {
            let gui_settings = gui_settings.clone();
            move || {
            vec![
                (
                    crate::gui::properties_panel::FIELD_PACKAGE_NAME,
                    gui_settings.get_flag(CFG_PACKAGE_NAME_VISIBLE, true).unwrap_or(true),
                ),
                (
                    crate::gui::properties_panel::FIELD_FILE_NAME,
                    gui_settings.get_flag(CFG_FILE_NAME_VISIBLE, true).unwrap_or(true),
                ),
                (
                    crate::gui::properties_panel::FIELD_SAVE_TO,
                    gui_settings.get_flag(CFG_SAVE_TO_VISIBLE, true).unwrap_or(true),
                ),
                (
                    crate::gui::properties_panel::FIELD_SOURCE,
                    gui_settings.get_flag(CFG_SOURCE_VISIBLE, false).unwrap_or(false),
                ),
                (
                    crate::gui::properties_panel::FIELD_COMMENT,
                    gui_settings.get_flag(CFG_COMMENT_VISIBLE, true).unwrap_or(true),
                ),
            ]
            }
        });
        // Properties panel only supports link-level detail for now: package
        // selections hide it, matching "no data to show" rather than guessing.
        selection.connect_selection_changed({
            let selection = selection.clone();
            let properties = properties.clone();
            move |_, _, _| {
                let bitset = selection.selection();
                if bitset.size() == 0 {
                    properties.update(None);
                    return;
                }
                let pos = bitset.nth(0);
                let Some(tree_row) = selection.item(pos).and_downcast::<gtk4::TreeListRow>()
                else {
                    properties.update(None);
                    return;
                };
                if tree_row.depth() == 0 {
                    properties.update(None);
                    return;
                }
                let Some(obj) = tree_row
                    .item()
                    .and_then(|i| i.downcast::<glib::BoxedAnyObject>().ok())
                else {
                    properties.update(None);
                    return;
                };
                let row = obj.borrow::<DownloadRow>();
                properties.update(Some(crate::gui::properties_panel::PropertiesData {
                    link_id: row.uuid.parse().unwrap_or(0),
                    package_id: row.package_uuid,
                    file_name: row.name.clone(),
                    file_icon: row.file_type_icon.clone(),
                    package_name: row.package_name.clone(),
                    save_to: row.save_to.clone(),
                    url: row.url.clone(),
                    comment: row.comment.clone(),
                    priority: row.priority.clone(),
                }));
            }
        });

        // Overview
        let overview = Rc::new(crate::gui::overview_panel::OverviewPanel::build(
            tr!("Overview").as_ref(),
            crate::gui::icon_key::ICON_DOWNLOAD,
            overview_field_defs(),
            overview_actions(gui_settings.clone()),
        ));
        page.append(&overview.widget);
        crate::gui::overview_panel::restore_field_visibility(
            overview.clone(),
            {
                let gui_settings = gui_settings.clone();
                move || gui_settings.is_ready()
            },
            {
                let gui_settings = gui_settings.clone();
                move || {
                    overview_field_defs()
                        .into_iter()
                        .map(|d| {
                            let visible = gui_settings.get_flag(d.cfg_key, d.default_visible).unwrap_or(d.default_visible);
                            (d.key, visible)
                        })
                        .collect()
                }
            },
        );

        // Bottom bar
        let bottom_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        bottom_bar.set_margin_start(6);
        bottom_bar.set_margin_end(6);
        bottom_bar.set_margin_top(3);
        bottom_bar.set_margin_bottom(3);
        bottom_bar.set_valign(gtk4::Align::Center);
        bottom_bar.set_size_request(-1, 24);

        // Add links
        let add_btn = gtk4::Button::builder()
            .child(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
                crate::gui::icon_key::ICON_ADD,
            )))
            .build();
        add_btn.set_has_frame(false);
        add_btn.set_tooltip_text(Some(tr!("Add links to linkgrabber").as_ref()));
        add_btn.set_size_request(24, 24);
        bottom_bar.append(&add_btn);

        let add_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        add_popover_box.set_margin_start(8);
        add_popover_box.set_margin_end(8);
        add_popover_box.set_margin_top(8);
        add_popover_box.set_margin_bottom(8);
        for label in [tr!("Add links"), tr!("Add container"), tr!("Paste links")] {
            let row = gtk4::Button::builder()
                .label(label.as_str())
                .has_frame(false)
                .halign(gtk4::Align::Start)
                .build();
            add_popover_box.append(&row);
        }
        let add_popover = gtk4::Popover::new();
        add_popover.set_child(Some(&add_popover_box));
        let add_arrow = gtk4::MenuButton::new();
        add_arrow.set_child(Some(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
            crate::gui::icon_key::ICON_GO_DOWN,
        ))));
        add_arrow.set_popover(Some(&add_popover));
        add_arrow.set_size_request(12, 24);
        bottom_bar.append(&add_arrow);

        bottom_bar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

        // Delete
        let delete_btn = gtk4::Button::builder()
            .child(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
                crate::gui::icon_key::ICON_TRASH,
            )))
            .build();
        delete_btn.set_has_frame(false);
        delete_btn.set_tooltip_text(Some(tr!("Delete all").as_ref()));
        delete_btn.set_size_request(24, 24);
        bottom_bar.append(&delete_btn);

        let delete_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        delete_popover_box.set_margin_start(8);
        delete_popover_box.set_margin_end(8);
        delete_popover_box.set_margin_top(8);
        delete_popover_box.set_margin_bottom(8);
        for label in [
            tr!("Delete disabled"),
            tr!("Delete failed"),
            tr!("Delete finished"),
            tr!("Delete offline"),
        ] {
            let row = gtk4::Button::builder()
                .label(label.as_str())
                .has_frame(false)
                .halign(gtk4::Align::Start)
                .build();
            delete_popover_box.append(&row);
        }
        let delete_popover = gtk4::Popover::new();
        delete_popover.set_child(Some(&delete_popover_box));
        let delete_arrow = gtk4::MenuButton::new();
        delete_arrow.set_child(Some(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
            crate::gui::icon_key::ICON_GO_DOWN,
        ))));
        delete_arrow.set_popover(Some(&delete_popover));
        delete_arrow.set_size_request(12, 24);
        bottom_bar.append(&delete_arrow);

        bottom_bar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

        // Search
        let search = gtk4::SearchEntry::new();
        search.set_hexpand(true);
        search.set_placeholder_text(Some(tr!("Filter").as_ref()));
        search.set_size_request(80, 24);
        bottom_bar.append(&search);

        // Quick filter
        let filter = gtk4::DropDown::from_strings(&[
            "All", "Running", "Failed", "Exists", "Offline", "Skipped", "Successful", "Todo",
        ]);
        filter.set_size_request(80, 24);
        bottom_bar.append(&filter);

        // Quick settings
        let settings_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        settings_popover_box.set_margin_start(8);
        settings_popover_box.set_margin_end(8);
        settings_popover_box.set_margin_top(8);
        settings_popover_box.set_margin_bottom(8);
        for label in [
            tr!("Chunks"),
            tr!("Parallel downloads"),
            tr!("Parallel per host"),
            tr!("Speed limit"),
            tr!("Properties"),
            tr!("Overview"),
            tr!("Bottom bar manager"),
        ] {
            let row = gtk4::Button::builder()
                .label(label.as_str())
                .has_frame(false)
                .halign(gtk4::Align::Start)
                .build();
            settings_popover_box.append(&row);
        }
        let settings_popover = gtk4::Popover::new();
        settings_popover.set_child(Some(&settings_popover_box));
        let settings_btn = gtk4::MenuButton::new();
        settings_btn.set_child(Some(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
            crate::gui::icon_key::ICON_SETTINGS,
        ))));
        settings_btn.set_popover(Some(&settings_popover));
        settings_btn.set_size_request(24, 24);
        bottom_bar.append(&settings_btn);

        page.append(&bottom_bar);

        Self {
            widget: page,
            store,
            child_stores,
            selection,
            view,
            overview,
            expanded_state,
            package_live,
            speed_limited,
        }
    }

    /// Starts the periodic refresh loop (every 2 s).
    ///
    /// Fetches downloads from JDownloader, updates the package/link tree, and
    /// fires a notification when all active downloads complete.
    pub fn start_refresh(
        &self,
        api: Arc<JdApi>,
        window: adw::ApplicationWindow,
        app: adw::Application,
        toast_overlay: adw::ToastOverlay,
    ) {
        let view = self.view.clone();
        let store = self.store.clone();
        let child_stores = self.child_stores.clone();
        let selection = self.selection.clone();
        let overview = self.overview.clone();
        let expanded_state = self.expanded_state.clone();
        let package_live = self.package_live.clone();
        let speed_limited = self.speed_limited.clone();
        let last_packages: Rc<RefCell<Vec<PackageRow>>> = Rc::new(RefCell::new(Vec::new()));
        let last_links_by_package: Rc<RefCell<HashMap<i64, Vec<DownloadRow>>>> =
            Rc::new(RefCell::new(HashMap::new()));
        let had_running = Rc::new(Cell::new(false));
        let fetching: Arc<Mutex<std::collections::HashSet<String>>> =
            Arc::new(Mutex::new(std::collections::HashSet::new()));
        let icon_fetching: Arc<Mutex<std::collections::HashSet<String>>> =
            Arc::new(Mutex::new(std::collections::HashSet::new()));

        glib::source::timeout_add_local(Duration::from_secs(2), move || {
            let api = api.clone();
            let view = view.clone();
            let store = store.clone();
            let child_stores = child_stores.clone();
            let selection = selection.clone();
            let overview = overview.clone();
            let expanded_state = expanded_state.clone();
            let package_live = package_live.clone();
            let speed_limited = speed_limited.clone();
            let last_packages = last_packages.clone();
            let last_links_by_package = last_links_by_package.clone();
            let had_running = had_running.clone();
            let fetching = fetching.clone();
            let icon_fetching = icon_fetching.clone();
            let window = window.clone();
            let app = app.clone();
            let toast_overlay = toast_overlay.clone();

            let (tx, rx) = async_channel::bounded::<(Vec<Value>, Vec<Value>, bool)>(1);
            let api_fetch = api.clone();
            std::thread::spawn(move || {
                if let Ok((packages, links)) = api_fetch.query_downloads() {
                    let limit_enabled = GeneralSettings::new(api_fetch.clone())
                        .get_download_speed_limit_enabled()
                        .unwrap_or(false);
                    let _ = tx.try_send((packages, links, limit_enabled));
                }
            });

            glib::MainContext::default().spawn_local(async move {
                if let Ok((packages_json, links_json, limit_enabled)) = rx.recv().await {
                    speed_limited.set(limit_enabled);
                    update_overview_values(&overview, packages_json.len(), &links_json);
                    let new_packages: Vec<PackageRow> =
                        packages_json.iter().map(package_row_from_json).collect();
                    let new_links: Vec<DownloadRow> =
                        links_json.iter().map(row_from_json).collect();

                    // Trigger favicon downloads for hosts not yet cached.
                    for row in &new_links {
                        let host = &row.host_name;
                        if !host.is_empty()
                            && crate::gui::jd_icon::resolve_path(host).is_none()
                            && crate::gui::favicon::cached(host).is_none()
                            && !fetching.lock().unwrap().contains(host)
                        {
                            fetching.lock().unwrap().insert(host.clone());
                            let host = host.clone();
                            let api = api.clone();
                            let fetching = fetching.clone();
                            std::thread::spawn(move || {
                                if let Err(e) = crate::gui::favicon::resolve(&api, &host) {
                                    log::warn!("favicon for {}: {}", host, e);
                                }
                                fetching.lock().unwrap().remove(&host);
                            });
                        }
                    }

                    // Trigger fetches for status icon keys that couldn't be
                    // resolved locally: JDownloader hands out an opaque
                    // `"kc.<hash>"` key (a server-composited progress/merged
                    // icon) for `statusIconKey` whenever a link or package
                    // is actively downloading, with no bundled PNG
                    // equivalent — see `remote_icon`.
                    for key in packages_json
                        .iter()
                        .chain(links_json.iter())
                        .filter_map(|v| v.get("statusIconKey").and_then(Value::as_str))
                    {
                        if key.is_empty()
                            || crate::gui::remote_icon::resolve(key).is_some()
                            || icon_fetching.lock().unwrap().contains(key)
                        {
                            continue;
                        }
                        icon_fetching.lock().unwrap().insert(key.to_string());
                        let key = key.to_string();
                        let api = api.clone();
                        let icon_fetching = icon_fetching.clone();
                        std::thread::spawn(move || {
                            if let Err(e) = crate::gui::remote_icon::fetch(&api, &key) {
                                log::warn!("status icon for {}: {}", key, e);
                            }
                            icon_fetching.lock().unwrap().remove(&key);
                        });
                    }

                    // "All downloads complete" notification.
                    let any_running = new_links.iter().any(|r| r.running);
                    let any_finished = new_links.iter().any(|r| r.finished);
                    if had_running.get() && !any_running && any_finished {
                        let count = new_links.iter().filter(|r| r.finished).count();
                        super::notifications::send(
                            &window,
                            &app,
                            &toast_overlay,
                            tr!("Downloads complete").as_ref(),
                            tr!("One file downloaded successfully"
                                | "{n} files downloaded successfully" % count)
                                .as_ref(),
                        );
                    }
                    had_running.set(any_running);

                    // Capture the current selection (by identity) before
                    // touching any store, since positions inside a child
                    // store aren't the same coordinate space as the
                    // flattened tree the selection model exposes.
                    let (selected_packages, selected_links): (
                        std::collections::HashSet<i64>,
                        std::collections::HashSet<String>,
                    ) = {
                        let mut packages = std::collections::HashSet::new();
                        let mut links = std::collections::HashSet::new();
                        let bitset = selection.selection();
                        for i in 0..bitset.size() {
                            let pos = bitset.nth(i as u32);
                            let Some(tree_row) =
                                selection.item(pos).and_downcast::<gtk4::TreeListRow>()
                            else {
                                continue;
                            };
                            let Some(obj) = tree_row
                                .item()
                                .and_then(|i| i.downcast::<glib::BoxedAnyObject>().ok())
                            else {
                                continue;
                            };
                            if tree_row.depth() == 0 {
                                packages.insert(obj.borrow::<PackageRow>().uuid);
                            } else {
                                links.insert(obj.borrow::<DownloadRow>().uuid.clone());
                            }
                        }
                        (packages, links)
                    };

                    let mut links_by_package: HashMap<i64, Vec<DownloadRow>> = HashMap::new();
                    for row in new_links {
                        links_by_package.entry(row.package_uuid).or_default().push(row);
                    }

                    // sync_store/sync_store_keep_expanded rebuild rows via
                    // remove_all()+re-append when the list changes, which
                    // tears down the GtkListItem holding keyboard focus (if
                    // the user had clicked a row). GTK doesn't reassign focus
                    // on its own, so without this the Delete shortcut wired
                    // on `view` silently stops receiving events a few
                    // seconds after the row is touched.
                    let had_focus = view
                        .root()
                        .and_then(|r| r.focus())
                        .is_some_and(|f| f == view.clone().upcast::<gtk4::Widget>() || f.is_ancestor(&view));

                    sync_store_keep_expanded(
                        &store,
                        &mut last_packages.borrow_mut(),
                        new_packages,
                        |p| p.uuid,
                        |old, new| {
                            expanded_state.borrow().get(&new.uuid).copied().unwrap_or(false)
                                // A package finishing/restarting is worth a
                                // real rebind even while expanded — see
                                // `sync_store_keep_expanded`'s doc comment.
                                && old.running == new.running
                                && old.finished == new.finished
                        },
                        |p| {
                            for live in &package_live {
                                if let Some(update) = live.borrow().get(&p.uuid) {
                                    update(p);
                                }
                            }
                        },
                    );

                    // Drop child stores/history for packages that no longer exist.
                    let current_uuids: std::collections::HashSet<i64> =
                        links_by_package.keys().copied().collect();
                    child_stores
                        .borrow_mut()
                        .retain(|uuid, _| current_uuids.contains(uuid));
                    last_links_by_package
                        .borrow_mut()
                        .retain(|uuid, _| current_uuids.contains(uuid));

                    for (package_uuid, links) in links_by_package {
                        let child_store = child_stores
                            .borrow_mut()
                            .entry(package_uuid)
                            .or_insert_with(|| {
                                gio::ListStore::builder()
                                    .item_type(glib::BoxedAnyObject::static_type())
                                    .build()
                            })
                            .clone();
                        let mut last = last_links_by_package
                            .borrow_mut()
                            .remove(&package_uuid)
                            .unwrap_or_default();
                        sync_store(&child_store, &mut last, links, |l| l.uuid.clone());
                        last_links_by_package.borrow_mut().insert(package_uuid, last);
                    }

                    // Restore the previous selection by identity.
                    if !selected_packages.is_empty() || !selected_links.is_empty() {
                        for pos in 0..selection.n_items() {
                            let Some(tree_row) =
                                selection.item(pos).and_downcast::<gtk4::TreeListRow>()
                            else {
                                continue;
                            };
                            let Some(obj) = tree_row
                                .item()
                                .and_then(|i| i.downcast::<glib::BoxedAnyObject>().ok())
                            else {
                                continue;
                            };
                            let matches = if tree_row.depth() == 0 {
                                selected_packages.contains(&obj.borrow::<PackageRow>().uuid)
                            } else {
                                selected_links.contains(&obj.borrow::<DownloadRow>().uuid)
                            };
                            if matches {
                                selection.select_item(pos, false);
                            }
                        }
                    }

                    if had_focus {
                        view.grab_focus();
                    }
                }
            });
            glib::ControlFlow::Continue
        });
    }

    /// All package uuids, top-to-bottom in the order JDownloader returned
    /// them (== the order shown, since [`sync_store`] never reorders items
    /// on its own). Used by the toolbar's move actions to compute a target
    /// package for `movePackages`'s `afterDestPackageId`.
    pub fn all_package_uuids(&self) -> Vec<i64> {
        (0..self.store.n_items())
            .filter_map(|pos| {
                self.store
                    .item(pos)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .map(|obj| obj.borrow::<PackageRow>().uuid)
            })
            .collect()
    }

    /// Currently selected package uuids (depth-0 tree rows only), in
    /// top-to-bottom order.
    pub fn selected_package_uuids(&self) -> Vec<i64> {
        let bitset = self.selection.selection();
        (0..bitset.size())
            .filter_map(|i| {
                let pos = bitset.nth(i as u32);
                let tree_row = self.selection.item(pos).and_downcast::<gtk4::TreeListRow>()?;
                if tree_row.depth() != 0 {
                    return None;
                }
                let obj = tree_row.item()?.downcast::<glib::BoxedAnyObject>().ok()?;
                let uuid = obj.borrow::<PackageRow>().uuid;
                Some(uuid)
            })
            .collect()
    }
}

/// Walks the flattened tree and collects `(uuid, bytes_loaded)` for every
/// row that should be removed for the current selection: individually
/// selected link rows, plus (mirroring JDownloader) every link belonging to
/// a selected package row, since deleting a package deletes all files
/// inside it.
fn selected_rows(
    selection: &gtk4::MultiSelection,
    child_stores: &Rc<RefCell<HashMap<i64, gio::ListStore>>>,
) -> Vec<(String, i64)> {
    let bitset = selection.selection();
    let stores = child_stores.borrow();
    let mut seen = std::collections::HashSet::new();
    let mut rows = Vec::new();
    let mut push = |uuid: String, bytes_loaded: i64| {
        if seen.insert(uuid.clone()) {
            rows.push((uuid, bytes_loaded));
        }
    };
    for i in 0..bitset.size() {
        let pos = bitset.nth(i as u32);
        let Some(tree_row) = selection.item(pos).and_downcast::<gtk4::TreeListRow>() else {
            continue;
        };
        if tree_row.depth() == 0 {
            let Some(obj) = tree_row.item().and_downcast::<glib::BoxedAnyObject>() else {
                continue;
            };
            let package_uuid = obj.borrow::<PackageRow>().uuid;
            if let Some(store) = stores.get(&package_uuid) {
                for cpos in 0..store.n_items() {
                    if let Some(child) = store.item(cpos).and_downcast::<glib::BoxedAnyObject>() {
                        let row = child.borrow::<DownloadRow>();
                        push(row.uuid.clone(), row.bytes_loaded);
                    }
                }
            }
        } else if let Some(obj) = tree_row.item().and_downcast::<glib::BoxedAnyObject>() {
            let row = obj.borrow::<DownloadRow>();
            push(row.uuid.clone(), row.bytes_loaded);
        }
    }
    rows
}

/// Selected link/package ids for archive-related actions
/// (`extraction/startExtractionNow`, `extraction/getArchiveInfo`, etc.),
/// which take `linkIds`/`packageIds` separately rather than always
/// resolving a package down to its children: a selected package row
/// contributes its own uuid to `package_ids` (letting JDownloader resolve
/// its archives itself), while a selected link row contributes its uuid,
/// parsed back to the `i64` it always was (see `row_from_json`), to
/// `link_ids`.
pub(crate) fn selected_archive_ids(
    selection: &gtk4::MultiSelection,
) -> (Vec<i64>, Vec<i64>) {
    let bitset = selection.selection();
    let mut link_ids = Vec::new();
    let mut package_ids = Vec::new();
    for i in 0..bitset.size() {
        let pos = bitset.nth(i as u32);
        let Some(tree_row) = selection.item(pos).and_downcast::<gtk4::TreeListRow>() else {
            continue;
        };
        let Some(obj) = tree_row.item().and_downcast::<glib::BoxedAnyObject>() else {
            continue;
        };
        if tree_row.depth() == 0 {
            package_ids.push(obj.borrow::<PackageRow>().uuid);
        } else if let Ok(id) = obj.borrow::<DownloadRow>().uuid.parse::<i64>() {
            link_ids.push(id);
        }
    }
    (link_ids, package_ids)
}

/// Removes link rows matching `uuids` from whichever per-package child store
/// currently holds them.
fn remove_rows_by_uuid(
    child_stores: &Rc<RefCell<HashMap<i64, gio::ListStore>>>,
    uuids: &std::collections::HashSet<String>,
) {
    for store in child_stores.borrow().values() {
        let mut pos = store.n_items();
        while pos > 0 {
            pos -= 1;
            if let Some(obj) = store.item(pos).and_downcast::<glib::BoxedAnyObject>() {
                if uuids.contains(&obj.borrow::<DownloadRow>().uuid) {
                    store.remove(pos);
                }
            }
        }
    }
}

/// Removes from the top-level store any package whose child store is now
/// empty, so a package that was fully deleted disappears immediately
/// instead of lingering as an empty row until the next periodic refresh.
fn prune_empty_packages(
    store: &gio::ListStore,
    child_stores: &Rc<RefCell<HashMap<i64, gio::ListStore>>>,
) {
    let mut pos = store.n_items();
    while pos > 0 {
        pos -= 1;
        if let Some(obj) = store.item(pos).and_downcast::<glib::BoxedAnyObject>() {
            let uuid = obj.borrow::<PackageRow>().uuid;
            let empty = child_stores
                .borrow()
                .get(&uuid)
                .is_some_and(|s| s.n_items() == 0);
            if empty {
                store.remove(pos);
                child_stores.borrow_mut().remove(&uuid);
            }
        }
    }
}

/// Selects and focuses the flattened-tree row at `pos` (clamped to the last
/// row once items have been removed), so after deleting a selection the row
/// that slid up to fill its place ends up selected and focused — mirroring
/// how most file managers handle "delete the selected item(s)".
pub fn select_and_focus_row(view: &gtk4::ColumnView, selection: &gtk4::MultiSelection, pos: u32) {
    let n = selection.n_items();
    if n == 0 {
        return;
    }
    let target = pos.min(n - 1);
    selection.select_item(target, true);
    // `ListScrollFlags::FOCUS` targets the row widget directly, but that
    // widget may not be realized/mapped yet right after the model changes,
    // in which case the focus request is silently dropped — the row still
    // *looks* selected, but keyboard events (e.g. a second Delete) no
    // longer reach it. Scroll the row into view, then grab focus on the
    // already-mapped `view` itself, which GTK forwards to the selected row;
    // this is the same reliable pattern used to restore focus after the
    // periodic refresh.
    view.scroll_to(target, None, gtk4::ListScrollFlags::SELECT, None);
    view.grab_focus();
}

/// `DeleteFileOptions` enum names, as used by JDownloader's `/downloadsV2/cleanup`.
const MODE_REMOVE_LINKS_ONLY: &str = "REMOVE_LINKS_ONLY";
const MODE_RECYCLE_FILES: &str = "REMOVE_LINKS_AND_RECYCLE_FILES";
const MODE_DELETE_FILES: &str = "REMOVE_LINKS_AND_DELETE_FILES";

/// Config key for the "Don't show this again" checkbox on this dialog
/// (mirrors JDownloader's per-dialog dont-show-again registry).
const SKIP_CONFIRM_KEY: &str = "skip_confirm_downloads_remove";

/// Shows JDownloader's "Are you sure?" removal dialog: a summary of what will
/// be removed, plus (only when some data was already downloaded) a dropdown
/// to also recycle or permanently delete the files on disk. Always shown
/// unless the user previously ticked "Don't show this again" — in that case
/// files are only detached from the list, never silently recycled/deleted.
/// Calls `on_confirm` with the chosen delete mode if the user confirms.
///
/// Uses a plain `gtk4::Window` rather than `adw::AlertDialog` so the "Don't
/// show this again" checkbox can sit on the same row as the Continue/Cancel
/// buttons, matching JDownloader's layout (`AlertDialog`'s response row is
/// fixed and can't host extra widgets).
fn confirm_remove<W: IsA<gtk4::Widget> + Clone + 'static>(
    parent: &W,
    link_count: usize,
    bytes_loaded: i64,
    local_file_count: usize,
    links_left: usize,
    on_confirm: impl FnOnce(&'static str) + 'static,
) {
    if crate::config::get_bool_map("dialog_prefs")
        .get(SKIP_CONFIRM_KEY)
        .copied()
        .unwrap_or(false)
    {
        on_confirm(MODE_REMOVE_LINKS_ONLY);
        return;
    }

    let Some(parent_window) = parent.root().and_then(|r| r.downcast::<gtk4::Window>().ok())
    else {
        on_confirm(MODE_REMOVE_LINKS_ONLY);
        return;
    };

    let dialog = gtk4::Window::new();
    dialog.set_transient_for(Some(&parent_window));
    dialog.set_modal(true);
    dialog.set_resizable(false);
    dialog.set_title(Some(tr!("Are you sure?").as_ref()));

    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(12);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let robot = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
        crate::gui::icon_key::ICON_BOTTY_STOP,
    ));
    robot.set_pixel_size(100);
    robot.set_valign(gtk4::Align::Start);
    content.append(&robot);

    let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    text_box.set_hexpand(true);

    let intro_label = gtk4::Label::new(Some(&format!(
        "{}\n{}",
        tr!("Do you really want to perform this clean up action:"),
        tr!("Delete Selected Downloads?")
    )));
    intro_label.set_halign(gtk4::Align::Start);
    intro_label.set_xalign(0.0);
    intro_label.set_wrap(true);
    intro_label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
    text_box.append(&intro_label);

    let tasks_label = gtk4::Label::new(Some(tr!("Tasks to do:").as_ref()));
    tasks_label.set_halign(gtk4::Align::Start);
    tasks_label.set_xalign(0.0);
    tasks_label.add_css_class("heading");
    tasks_label.set_margin_top(6);
    text_box.append(&tasks_label);

    let mut task_text = format!(
        "{} — {}",
        tr!("Delete selected link" | "Delete selected links" % link_count),
        tr!("{n} link remaining" | "{n} links remaining" % links_left),
    );
    if local_file_count > 0 {
        task_text.push('\n');
        task_text.push_str(&tr!(
            "{n} file ({}) on disk" | "{n} files ({}) on disk" % local_file_count,
            format_size_jd(bytes_loaded)
        ));
    }
    let task_label = gtk4::Label::new(Some(&task_text));
    task_label.set_halign(gtk4::Align::Start);
    task_label.set_xalign(0.0);
    task_label.set_wrap(true);
    task_label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
    text_box.append(&task_label);

    let mode_dropdown = if bytes_loaded > 0 {
        let options = [
            tr!("Keep all downloaded files - just remove links from JDownloader").to_string(),
            tr!("Move downloaded files to Trash & remove links from JDownloader").to_string(),
            tr!("Delete downloaded files permanently from disk & remove links from JDownloader")
                .to_string(),
        ];
        let option_refs: Vec<&str> = options.iter().map(String::as_str).collect();
        let dropdown = gtk4::DropDown::from_strings(&option_refs);
        dropdown.set_selected(0);
        dropdown.set_margin_top(6);
        text_box.append(&dropdown);
        Some(dropdown)
    } else {
        None
    };

    content.append(&text_box);
    outer.append(&content);

    let button_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    button_row.set_margin_top(6);
    button_row.set_margin_bottom(12);
    button_row.set_margin_start(18);
    button_row.set_margin_end(18);

    let dont_show_again = gtk4::CheckButton::with_label(tr!("Don't show this again").as_ref());
    dont_show_again.set_valign(gtk4::Align::Center);
    dont_show_again.set_hexpand(true);
    dont_show_again.set_halign(gtk4::Align::Start);
    button_row.append(&dont_show_again);

    let cancel_btn = gtk4::Button::with_label(tr!("Cancel").as_ref());
    let continue_btn = gtk4::Button::with_label(tr!("Continue").as_ref());
    continue_btn.add_css_class("suggested-action");

    let button_size_group = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
    button_size_group.add_widget(&cancel_btn);
    button_size_group.add_widget(&continue_btn);

    let buttons_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    buttons_box.set_halign(gtk4::Align::End);
    buttons_box.append(&continue_btn);
    buttons_box.append(&cancel_btn);
    button_row.append(&buttons_box);
    outer.append(&button_row);

    dialog.set_child(Some(&outer));
    dialog.set_default_widget(Some(&continue_btn));

    let escape_controller = gtk4::EventControllerKey::new();
    escape_controller.connect_key_pressed({
        let cancel_btn = cancel_btn.clone();
        move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                cancel_btn.activate();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    dialog.add_controller(escape_controller);

    cancel_btn.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });

    let on_confirm = Rc::new(RefCell::new(Some(on_confirm)));
    let dialog_c = dialog.clone();
    continue_btn.connect_clicked(move |_| {
        if dont_show_again.is_active() {
            crate::config::set_bool_entry("dialog_prefs", SKIP_CONFIRM_KEY, true);
        }
        let mode = match mode_dropdown.as_ref().map(|d| d.selected()) {
            Some(1) => MODE_RECYCLE_FILES,
            Some(2) => MODE_DELETE_FILES,
            _ => MODE_REMOVE_LINKS_ONLY,
        };
        if let Some(f) = on_confirm.borrow_mut().take() {
            f(mode);
        }
        dialog_c.close();
    });

    dialog.present();
    continue_btn.grab_focus();
}

/// Removes the selected downloads from the list, mirroring JDownloader's
/// "Remove"/"Delete" action: prompts for confirmation (as JDownloader does)
/// and, depending on the chosen mode, either only detaches the entries from
/// the list or also recycles/permanently deletes the downloaded files.
pub fn remove_selected(
    api: &Arc<JdApi>,
    selection: &gtk4::MultiSelection,
    store: &gio::ListStore,
    child_stores: &Rc<RefCell<HashMap<i64, gio::ListStore>>>,
    view: &gtk4::ColumnView,
) {
    let rows = selected_rows(selection, child_stores);
    if rows.is_empty() {
        return;
    }
    let link_count = rows.len();
    let bytes_loaded: i64 = rows.iter().map(|(_, bytes)| *bytes).sum();
    let local_file_count = rows.iter().filter(|(_, bytes)| *bytes > 0).count();
    let links_left: usize = child_stores
        .borrow()
        .values()
        .map(|s| s.n_items() as usize)
        .sum::<usize>()
        - link_count;
    let uuids: std::collections::HashSet<String> =
        rows.into_iter().map(|(uuid, _)| uuid).collect();

    // Lowest flattened-tree position among the rows about to be removed:
    // once they're gone, the row that slides up into this position is the
    // one immediately below the deleted selection.
    let focus_pos = selection.selection().minimum();

    let api = api.clone();
    let store = store.clone();
    let child_stores = child_stores.clone();
    let selection = selection.clone();
    let view = view.clone();
    confirm_remove(
        &view.clone(),
        link_count,
        bytes_loaded,
        local_file_count,
        links_left,
        move |mode| {
            let ids: Vec<i64> = uuids.iter().filter_map(|u| u.parse().ok()).collect();
            crate::gui::spawn::api_call(
                move || {
                    let _ = api.cleanup_downloads(&ids, mode);
                },
                move |_| {
                    remove_rows_by_uuid(&child_stores, &uuids);
                    prune_empty_packages(&store, &child_stores);
                    select_and_focus_row(&view, &selection, focus_pos);
                },
            );
        },
    );
}

/// Picks a file-type icon key (video/audio/image/document/archive/generic)
/// from a file name's extension. Shared with `link_grabber_panel.rs`.
pub(crate) fn file_type_icon_key(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "mpeg" | "mpg" | "ts" | "m4v"
        | "3gp" => crate::gui::icon_key::ICON_VIDEO,
        "mp3" | "flac" | "wav" | "ogg" | "aac" | "m4a" | "wma" | "opus" | "ac3" | "dts"
        | "mka" => crate::gui::icon_key::ICON_AUDIO,
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "tiff" | "ico" | "raw"
        | "cr2" | "nef" => crate::gui::icon_key::ICON_IMAGE,
        "pdf" | "doc" | "docx" | "txt" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods"
        | "odp" | "rtf" | "csv" | "html" | "htm" | "xml" | "json" => {
            crate::gui::icon_key::ICON_DOCUMENT
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "tbz" | "lz" => {
            crate::gui::icon_key::ICON_RAR
        }
        _ => crate::gui::icon_key::ICON_FILE,
    }
}

/// Shared JSON pointer-based getters for both link and package payloads.
pub(crate) fn get_str_at(value: &Value, key: &str) -> String {
    value
        .pointer(&format!("/{}", key))
        .or_else(|| value.pointer(&format!("/infoMap/{}", key)))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn get_num_at(value: &Value, key: &str) -> Option<i64> {
    value
        .pointer(&format!("/{}", key))
        .or_else(|| value.pointer(&format!("/infoMap/{}", key)))
        .and_then(Value::as_i64)
}

pub(crate) fn get_bool_at(value: &Value, key: &str) -> bool {
    value
        .pointer(&format!("/{}", key))
        .or_else(|| value.pointer(&format!("/infoMap/{}", key)))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// A link is "failed" once JDownloader has attached a `FinalLinkState` that
/// isn't one of the `FINISHED*` variants — mirrors
/// `org.jdownloader.plugins.FinalLinkState.isFailed()` (`!isFinished()`).
fn is_failed_link(link: &Value) -> bool {
    link.pointer("/advancedStatus/FinalLinkState/id")
        .and_then(Value::as_str)
        .map(|id| !id.starts_with("FINISHED"))
        .unwrap_or(false)
}

/// Recomputes every Overview stat from the flat list of download links (the
/// same aggregation JDownloader's `AggregatedNumbers` does over its
/// `SelectionInfo`), and pushes the results into `overview`.
fn update_overview_values(overview: &crate::gui::overview_panel::OverviewPanel, packages_len: usize, links_json: &[Value]) {
    let mut bytes_total: i64 = 0;
    let mut bytes_loaded: i64 = 0;
    let mut speed: i64 = 0;
    let mut running = 0i64;
    let mut finished = 0i64;
    let mut skipped = 0i64;
    let mut failed = 0i64;
    for link in links_json {
        bytes_total += get_num_at(link, "bytesTotal").unwrap_or(0);
        bytes_loaded += get_num_at(link, "bytesLoaded").unwrap_or(0);
        if get_bool_at(link, "running") {
            running += 1;
            speed += get_num_at(link, "speed").unwrap_or(0);
        }
        if get_bool_at(link, "finished") {
            finished += 1;
        }
        if get_bool_at(link, "skipped") {
            skipped += 1;
        }
        if is_failed_link(link) {
            failed += 1;
        }
    }
    let remaining = (bytes_total - bytes_loaded).max(0);
    let eta = if speed > 0 {
        format_seconds(remaining / speed)
    } else {
        String::from("-")
    };

    overview.set_value(OVERVIEW_PACKAGES, &packages_len.to_string());
    overview.set_value(OVERVIEW_LINKS, &links_json.len().to_string());
    overview.set_value(OVERVIEW_SIZE, &format_size_jd(bytes_total));
    overview.set_value(OVERVIEW_SPEED, &format!("{}/s", format_size_jd(speed)));
    overview.set_value(OVERVIEW_LOADED, &format_size_jd(bytes_loaded));
    overview.set_value(OVERVIEW_REMAINING, &format_size_jd(remaining));
    overview.set_value(OVERVIEW_ETA, &eta);
    overview.set_value(OVERVIEW_RUNNING, &running.to_string());
    overview.set_value(OVERVIEW_FINISHED, &finished.to_string());
    overview.set_value(OVERVIEW_SKIPPED, &skipped.to_string());
    overview.set_value(OVERVIEW_FAILED, &failed.to_string());
}

fn format_seconds(seconds: i64) -> String {
    if seconds < 0 {
        return String::new();
    }
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn package_row_from_json(pkg: &Value) -> PackageRow {
    let uuid = get_num_at(pkg, "uuid").unwrap_or(0);
    let name = get_str_at(pkg, "name");
    let save_to = get_str_at(pkg, "saveTo");
    let child_count = get_num_at(pkg, "childCount").unwrap_or(0);
    let bytes_total = get_num_at(pkg, "bytesTotal").unwrap_or(0);
    let bytes_loaded = get_num_at(pkg, "bytesLoaded").unwrap_or(0);
    let running = get_bool_at(pkg, "running");
    let status = get_str_at(pkg, "status");
    let status_icon_key = get_str_at(pkg, "statusIconKey");
    let status_icon_resolved = crate::gui::remote_icon::resolve(&status_icon_key);
    let has_status_icon = status_icon_resolved.is_some();
    let speed_text = if running {
        get_num_at(pkg, "speed")
            .map(|s| format!("{}/s", format_size_jd(s)))
            .unwrap_or_default()
    } else {
        String::new()
    };
    PackageRow {
        uuid,
        name,
        save_to,
        child_count,
        bytes_total,
        bytes_loaded,
        size_text: format_size_jd(bytes_total),
        loaded_text: format_size_jd(bytes_loaded),
        status,
        status_icon: status_icon_resolved
            .unwrap_or_else(|| crate::gui::jd_icon::resolve(&status_icon_key)),
        has_status_icon,
        speed_text,
        eta_text: get_num_at(pkg, "eta").map(format_seconds).unwrap_or_default(),
        enabled: get_bool_at(pkg, "enabled"),
        finished: get_bool_at(pkg, "finished"),
        running,
        comment: get_str_at(pkg, "comment"),
        extraction_progress: get_num_at(pkg, "extractionCurrent")
            .zip(get_num_at(pkg, "extractionTotal")),
    }
}

fn row_from_json(link: &Value) -> DownloadRow {
    let get_str = |key: &str| get_str_at(link, key);
    let get_num = |key: &str| get_num_at(link, key);
    let get_bool = |key: &str| get_bool_at(link, key);
    let name = get_str("name");
    let host = get_str("host");
    let uuid = get_num("uuid").map(|n| n.to_string()).unwrap_or_default();
    let package_uuid = get_num("packageUUID").unwrap_or(0);
    let package_name = get_str("packageName");
    let url = get_str("url");
    let priority = {
        let p = get_str("priority");
        if p.is_empty() { "DEFAULT".to_string() } else { p }
    };
    let bytes_total = get_num("bytesTotal").unwrap_or(0);
    let bytes_loaded = get_num("bytesLoaded").unwrap_or(0);
    let size_text = format_size_jd(bytes_total);
    let status = get_str("status");
    let status_icon_key = get_str("statusIconKey");
    let status_icon_resolved = crate::gui::remote_icon::resolve(&status_icon_key);
    let has_status_icon = status_icon_resolved.is_some();
    let running = get_bool("running");
    let speed_text = if running {
        get_num("speed")
            .map(|s| format!("{}/s", format_size_jd(s)))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let eta_text = get_num("eta").map(format_seconds).unwrap_or_default();
    let loaded_text = format_size_jd(bytes_loaded);
    let save_to = get_str("downloadPath");
    let comment = get_str("comment");
    let skipped = get_bool("skipped");
    let finished = get_bool("finished");
    let enabled = get_bool("enabled");
    let show_progress = link
        .get("advancedStatus")
        .and_then(Value::as_object)
        .map(|adv| {
            let priority = [
                "PluginProgress",
                "ConditionalSkipReason",
                "SkipReason",
                "FinalLinkState",
                "ExtractionStatus",
                "SingleDownloadController",
                "AvailableStatus",
                "LinkCrawlerRetry",
            ];
            match priority.iter().copied().find(|k| adv.contains_key(*k)) {
                Some("PluginProgress") => true,
                Some("SingleDownloadController") => true,
                Some("ConditionalSkipReason") => true,
                Some("ExtractionStatus") => adv
                    .get("ExtractionStatus")
                    .and_then(|v| v.get("id"))
                    .and_then(Value::as_str)
                    == Some("RUNNING"),
                _ => false,
            }
        })
        .unwrap_or(false);

    // JDownloader reuses the link's own `PluginProgress` for the
    // extraction phase too (`ExtractionProgress`, tagged `id: "EXTRACTION"`
    // — verified against the SVN's `ProgressColumn`/`DownloadLinkArchiveFile`),
    // with `current`/`total` counting decompressed bytes rather than
    // downloaded ones; `id: "DOWNLOAD"` is the ordinary download-progress
    // case, already covered by `bytes_loaded`/`bytes_total`.
    let extraction_progress = link
        .pointer("/advancedStatus/PluginProgress")
        .filter(|p| p.get("id").and_then(Value::as_str) == Some("EXTRACTION"))
        .and_then(|p| p.get("current").and_then(Value::as_i64).zip(p.get("total").and_then(Value::as_i64)));

    let has_host_icon = !host.is_empty() && crate::gui::jd_icon::resolve_path(&host).is_some();

    DownloadRow {
        uuid,
        package_uuid,
        package_name,
        name: name.clone(),
        url,
        priority,
        file_type_icon: crate::gui::jd_icon::resolve(file_type_icon_key(&name)),
        host_icon: crate::gui::jd_icon::resolve_or(&host, crate::gui::icon_key::ICON_FILE),
        host_name: host,
        has_host_icon,
        bytes_total,
        bytes_loaded,
        size_text,
        status,
        status_icon: status_icon_resolved
            .unwrap_or_else(|| crate::gui::jd_icon::resolve(&status_icon_key)),
        has_status_icon,
        speed_text,
        eta_text,
        loaded_text,
        save_to,
        comment,
        skipped,
        running,
        finished,
        enabled,
        show_progress,
        extraction_progress,
    }
}

/// Formats a byte count the same way JDownloader's SizeFormatter does.
pub(crate) fn format_size_jd(bytes: i64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let abs = bytes.abs();
    let (value, unit) = if abs >= 1024i64.pow(4) {
        (bytes as f64 / 1024f64.powi(4), "TiB")
    } else if abs >= 1024i64.pow(3) {
        (bytes as f64 / 1024f64.powi(3), "GiB")
    } else if abs >= 1024i64.pow(2) {
        (bytes as f64 / 1024f64.powi(2), "MiB")
    } else if abs >= 1024 {
        (bytes as f64 / 1024f64, "KiB")
    } else {
        return format!("{} B", bytes);
    };
    format!("{} {}", format_float(value), unit)
}

fn format_float(value: f64) -> String {
    let s = format!("{:.2}", value);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
