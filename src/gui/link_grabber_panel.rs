use std::cell::RefCell;
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
use crate::jd::{GraphicalUserInterfaceSettings, JdApi};

#[derive(Clone, Debug)]
pub struct LinkGrabberRow {
    pub file: String,
    pub file_icon: gtk4::gio::Icon,
    /// Current variant's display name (e.g. a video resolution/format), or
    /// empty when the link's plugin doesn't support variants.
    pub variant: String,
    pub variant_id: String,
    pub variant_icon: gtk4::gio::Icon,
    /// True when the variant's icon key actually resolved to a real image.
    /// Most variant icon keys aren't part of gDownloader's bundled/JD-theme
    /// icons, so `variant_icon` alone would render as a broken-image glyph.
    pub has_variant_icon: bool,
    pub save_to: String,
    pub size: String,
    pub host_icon: gtk4::gio::Icon,
    pub host_name: String,
    /// True when a real PNG was resolved for this host (false = using fallback icon).
    /// Included in PartialEq so the store rebuilds when a favicon download completes.
    pub has_host_icon: bool,
    pub avail_icon: gtk4::gio::Icon,
    pub avail_tooltip: String,
    pub uuid: String,
    pub package_uuid: i64,
    pub package_name: String,
    pub url: String,
    pub priority: String,
    pub comment: String,
}

impl PartialEq for LinkGrabberRow {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file
            && self.variant == other.variant
            && self.variant_id == other.variant_id
            && self.has_variant_icon == other.has_variant_icon
            && self.save_to == other.save_to
            && self.size == other.size
            && self.host_name == other.host_name
            && self.has_host_icon == other.has_host_icon
            && self.avail_tooltip == other.avail_tooltip
            && self.uuid == other.uuid
            && self.package_uuid == other.package_uuid
            && self.package_name == other.package_name
            && self.url == other.url
            && self.priority == other.priority
            && self.comment == other.comment
    }
}

/// A package (group) row: the collapsible parent of a set of
/// [`LinkGrabberRow`]s in the link grabber tree, mirroring JDownloader.
#[derive(Clone, Debug)]
pub struct LinkGrabberPackageRow {
    pub uuid: i64,
    pub name: String,
    pub save_to: String,
    pub child_count: i64,
    pub bytes_total: i64,
    pub size_text: String,
    pub enabled: bool,
    pub finished: bool,
    pub comment: String,
}

impl PartialEq for LinkGrabberPackageRow {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
            && self.name == other.name
            && self.save_to == other.save_to
            && self.child_count == other.child_count
            && self.bytes_total == other.bytes_total
            && self.size_text == other.size_text
            && self.enabled == other.enabled
            && self.finished == other.finished
            && self.comment == other.comment
    }
}

pub struct LinkGrabberPanel {
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
    /// `package_tree::sync_store_keep_expanded`).
    expanded_state: Rc<RefCell<HashMap<i64, bool>>>,
}

/// Keys identifying each Overview stat, passed to
/// `overview_panel::OverviewPanel::set_value`.
const OVERVIEW_PACKAGES: &str = "packages";
const OVERVIEW_LINKS: &str = "links";
const OVERVIEW_SIZE: &str = "size";
const OVERVIEW_ONLINE: &str = "online";
const OVERVIEW_HOSTER: &str = "hoster";
const OVERVIEW_OFFLINE: &str = "offline";
const OVERVIEW_UNKNOWN: &str = "unknown";

/// Overview stats and their JDownloader `GraphicalUserInterfaceSettings`
/// visibility flags (JDownloader's own key names, reused verbatim), in
/// `LinkgrabberOverview.createDataEntries()`'s declaration order — that
/// order drives the 2-row grid packing (see `overview_panel::relayout`).
/// Defaults mirror JDownloader's `@DefaultBooleanValue` on each
/// `is...Visible()`.
fn overview_field_defs() -> Vec<crate::gui::overview_panel::OverviewFieldDef> {
    use crate::gui::overview_panel::OverviewFieldDef;
    vec![
        OverviewFieldDef::new(
            OVERVIEW_PACKAGES,
            "OverviewPanelLinkgrabberPackageCountVisible",
            tr!("Packages").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_LINKS,
            "OverviewPanelLinkgrabberLinksCountVisible",
            tr!("Links").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_SIZE,
            "OverviewPanelLinkgrabberTotalBytesVisible",
            tr!("Size").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_ONLINE,
            "OverviewPanelLinkgrabberStatusOnlineVisible",
            tr!("Online").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_HOSTER,
            "OverviewPanelLinkgrabberHosterCountVisible",
            tr!("Hoster").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_OFFLINE,
            "OverviewPanelLinkgrabberStatusOfflineVisible",
            tr!("Offline").to_string(),
            true,
        ),
        OverviewFieldDef::new(
            OVERVIEW_UNKNOWN,
            "OverviewPanelLinkgrabberStatusUnknownVisible",
            tr!("Unknown").to_string(),
            true,
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
const CFG_PACKAGE_NAME_VISIBLE: &str = "LinkPropertiesPanelPackagenameVisible";
const CFG_FILE_NAME_VISIBLE: &str = "LinkPropertiesPanelFilenameVisible";
const CFG_SAVE_TO_VISIBLE: &str = "LinkPropertiesPanelSaveToVisible";
const CFG_SOURCE_VISIBLE: &str = "LinkPropertiesPanelDownloadFromVisible";
const CFG_COMMENT_VISIBLE: &str = "LinkPropertiesPanelCommentVisible";

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
                    let _ = api.rename_linkgrabber_link(id, &name);
                });
            }
        }),
        rename_package: Box::new({
            let api = api.clone();
            move |id, name| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.rename_linkgrabber_package(id, &name);
                });
            }
        }),
        set_comment: Box::new({
            let api = api.clone();
            move |link_id, comment| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.set_linkgrabber_comment(&[link_id], &[], false, &comment);
                });
            }
        }),
        set_priority: Box::new({
            let api = api.clone();
            move |link_id, priority| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.set_linkgrabber_priority(&[link_id], &[], &priority);
                });
            }
        }),
        set_download_directory: Box::new({
            let api = api.clone();
            move |package_id, dir| {
                let api = api.clone();
                crate::gui::spawn::api_fire(move || {
                    let _ = api.set_linkgrabber_directory(&dir, &[package_id]);
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

impl LinkGrabberPanel {
    #[allow(deprecated)]
    pub fn build(api: Arc<JdApi>, gui_settings: GraphicalUserInterfaceSettings) -> Self {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        page.set_vexpand(true);
        page.set_hexpand(true);

        // Table
        let PackageTree {
            store,
            child_stores,
            expanded_state,
            selection,
            ..
        } = PackageTree::build::<LinkGrabberPackageRow>("linkgrabber", |p| p.uuid);

        let view = gtk4::ColumnView::new(Some(selection.clone()));
        view.set_vexpand(true);
        view.set_hexpand(true);
        view.set_show_row_separators(true);
        view.set_show_column_separators(false);

        // Text column showing `pkg_accessor(&LinkGrabberPackageRow)` for
        // package rows (depth 0) and `link_accessor(&LinkGrabberRow)` for
        // link rows (depth 1).
        let dual_text_col = |pkg_accessor: fn(&LinkGrabberPackageRow) -> &str,
                             link_accessor: fn(&LinkGrabberRow) -> &str,
                             xalign: f32,
                             min_width: i32,
                             title: &str,
                             resizable: bool,
                             expand: bool,
                             ellipsize: bool|
         -> gtk4::ColumnViewColumn {
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
            factory.connect_bind(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let Some((tree_row, obj)) = tree_item(list_item) else {
                    return;
                };
                let Some(label) = list_item.child().and_downcast::<gtk4::Label>() else {
                    return;
                };
                let text = if tree_row.depth() == 0 {
                    pkg_accessor(&obj.borrow::<LinkGrabberPackageRow>()).to_string()
                } else {
                    link_accessor(&obj.borrow::<LinkGrabberRow>()).to_string()
                };
                label.set_text(&text);
                if ellipsize {
                    label.set_tooltip_text(Some(&text));
                }
            });
            let col = gtk4::ColumnViewColumn::new(Some(title), Some(factory));
            col.set_fixed_width(min_width);
            col.set_resizable(resizable);
            col.set_expand(expand);
            col
        };

        // Columns the "choose visible columns" header menu can toggle (the
        // File column itself always stays visible, matching JDownloader).
        let mut togglable_columns: Vec<(&'static str, String, gtk4::ColumnViewColumn)> = Vec::new();

        // File: TreeExpander (package/link indentation + expand triangle) + icon + label.
        {
            let col = build_name_column::<LinkGrabberPackageRow, LinkGrabberRow>(
                tr!("File").as_ref(),
                140,
                "linkgrabber",
                expanded_state.clone(),
                |p| p.uuid,
                |p| &p.name,
                (|r| &r.file_icon, |r| &r.file),
            );
            view.append_column(&col);
        }
        // Variant: mirrors JDownloader's in-cell variant chooser (e.g. video
        // resolution/format). Shown only for links whose plugin supports
        // variants; clicking fetches the available options and lets the
        // user pick one.
        {
            let factory = gtk4::SignalListItemFactory::new();
            factory.connect_setup({
                let api = api.clone();
                move |_, list_item| {
                    let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap().clone();
                    let btn = gtk4::Button::new();
                    btn.set_has_frame(false);
                    btn.set_halign(gtk4::Align::Fill);
                    btn.set_hexpand(true);
                    btn.set_size_request(100, -1);
                    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
                    hbox.set_hexpand(true);
                    let icon = gtk4::Image::new();
                    icon.set_pixel_size(14);
                    let label = gtk4::Label::new(None);
                    label.set_halign(gtk4::Align::Start);
                    label.set_hexpand(true);
                    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    // Combo-style down arrow, always pinned at the end even
                    // when the label truncates, mirroring JDownloader's
                    // Variant column combobox (which uses IconKey.ICON_POPDOWNLARGE
                    // for its dropdown arrow, not the list-reorder "go-down" icon).
                    let arrow = gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(
                        crate::gui::icon_key::ICON_POPDOWNLARGE,
                    ));
                    arrow.set_pixel_size(10);
                    arrow.set_halign(gtk4::Align::End);
                    hbox.append(&icon);
                    hbox.append(&label);
                    hbox.append(&arrow);
                    btn.set_child(Some(&hbox));

                    // One popover per (recycled) button, parented once, so a
                    // slow/failed fetch on one row can never leave a stray
                    // half-parented popover behind on a button that gets
                    // rebound to a different row later.
                    let popover = gtk4::Popover::new();
                    popover.set_parent(&btn);

                    let api = api.clone();
                    let click_list_item = list_item.clone();
                    btn.connect_clicked(move |_| {
                        let Some((tree_row, obj)) = tree_item(&click_list_item) else {
                            return;
                        };
                        if tree_row.depth() == 0 {
                            return;
                        }
                        let row = obj.borrow::<LinkGrabberRow>();
                        let link_id: i64 = row.uuid.parse().unwrap_or(0);
                        let current_variant_id = row.variant_id.clone();
                        drop(row);
                        if link_id == 0 {
                            return;
                        }
                        let api_bg = api.clone();
                        let api_ui = api.clone();
                        let popover = popover.clone();
                        crate::gui::spawn::api_call(
                            move || api_bg.get_link_variants(link_id),
                            move |result| {
                                let api = api_ui;
                                let variants = match result {
                                    Ok(v) => v,
                                    Err(e) => {
                                        log::warn!("getVariants for link {}: {}", link_id, e);
                                        return;
                                    }
                                };
                                if variants.is_empty() {
                                    log::warn!("No variants returned for link {}", link_id);
                                    return;
                                }
                                // A ListBox rather than a plain Box: its rows
                                // pick up the usual hover/active highlight of
                                // menu entries, which a bare CheckButton
                                // doesn't get. The radio indicator itself is
                                // set non-target-able so pointer events hit
                                // the row (and its hover style) instead of
                                // being absorbed by the CheckButton.
                                let list_box = gtk4::ListBox::new();
                                list_box.set_selection_mode(gtk4::SelectionMode::None);
                                list_box.add_css_class("boxed-list");
                                list_box.set_margin_start(4);
                                list_box.set_margin_end(4);
                                list_box.set_margin_top(4);
                                list_box.set_margin_bottom(4);
                                // Grouped CheckButtons, one per variant, so
                                // the currently active variant shows
                                // pre-selected — mirroring JDownloader's
                                // variant chooser.
                                let mut group: Option<gtk4::CheckButton> = None;
                                for v in &variants {
                                    let id = v.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                                    let name = v.get("name").and_then(Value::as_str).unwrap_or("");
                                    let check = gtk4::CheckButton::builder()
                                        .can_target(false)
                                        .can_focus(false)
                                        .active(id == current_variant_id)
                                        .build();
                                    if let Some(leader) = &group {
                                        check.set_group(Some(leader));
                                    } else {
                                        group = Some(check.clone());
                                    }
                                    let api = api.clone();
                                    let popover_for_click = popover.clone();
                                    check.connect_toggled(move |check| {
                                        if !check.is_active() {
                                            return;
                                        }
                                        let api = api.clone();
                                        let id = id.clone();
                                        crate::gui::spawn::api_fire(move || {
                                            let _ = api.set_link_variant(link_id, &id);
                                        });
                                        popover_for_click.popdown();
                                    });

                                    let label = gtk4::Label::new(Some(name));
                                    label.set_halign(gtk4::Align::Start);
                                    label.set_hexpand(true);

                                    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                                    hbox.set_margin_start(4);
                                    hbox.set_margin_end(4);
                                    hbox.set_margin_top(2);
                                    hbox.set_margin_bottom(2);
                                    hbox.append(&check);
                                    hbox.append(&label);

                                    let list_row = gtk4::ListBoxRow::new();
                                    list_row.set_child(Some(&hbox));
                                    list_box.append(&list_row);
                                }
                                list_box.connect_row_activated(move |_, row| {
                                    let Some(hbox) = row.child().and_downcast::<gtk4::Box>() else {
                                        return;
                                    };
                                    let Some(check) =
                                        hbox.first_child().and_downcast::<gtk4::CheckButton>()
                                    else {
                                        return;
                                    };
                                    check.set_active(true);
                                });
                                // Scroll instead of growing unbounded: some
                                // hosts (e.g. YouTube) return dozens of
                                // variants, which without a cap render an
                                // oversized, effectively invisible popover —
                                // JDownloader's own variant menu scrolls too.
                                let scroll = gtk4::ScrolledWindow::builder()
                                    .child(&list_box)
                                    .max_content_height(320)
                                    .propagate_natural_height(true)
                                    .hscrollbar_policy(gtk4::PolicyType::Never)
                                    .build();
                                popover.set_child(Some(&scroll));
                                popover.popup();
                            },
                        );
                    });
                    list_item.set_child(Some(&btn));
                }
            });
            factory.connect_bind(move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
                let Some((tree_row, obj)) = tree_item(list_item) else {
                    return;
                };
                let Some(btn) = list_item.child().and_downcast::<gtk4::Button>() else {
                    return;
                };
                let Some(hbox) = btn.child().and_downcast::<gtk4::Box>() else {
                    return;
                };
                let Some(icon) = hbox.first_child().and_downcast::<gtk4::Image>() else {
                    return;
                };
                let Some(label) = icon.next_sibling().and_downcast::<gtk4::Label>() else {
                    return;
                };
                if tree_row.depth() == 0 {
                    btn.set_visible(false);
                    return;
                }
                let row = obj.borrow::<LinkGrabberRow>();
                if row.variant_id.is_empty() {
                    btn.set_visible(false);
                    return;
                }
                btn.set_visible(true);
                icon.set_visible(row.has_variant_icon);
                if row.has_variant_icon {
                    icon.set_from_gicon(&row.variant_icon);
                }
                label.set_text(&row.variant);
            });
            let col = gtk4::ColumnViewColumn::new(Some(tr!("Variant").as_ref()), Some(factory));
            col.set_expand(true);
            togglable_columns.push(("variant", tr!("Variant").to_string(), col.clone()));
            view.append_column(&col);
        }
        let save_to_col = dual_text_col(
            |p| &p.save_to,
            |r| &r.save_to,
            0.0,
            140,
            tr!("Save To").as_ref(),
            false,
            true,
            true,
        );
        togglable_columns.push(("save_to", tr!("Save To").to_string(), save_to_col.clone()));
        view.append_column(&save_to_col);

        let size_col = dual_text_col(
            |p| &p.size_text,
            |r| &r.size,
            1.0,
            90,
            tr!("Size").as_ref(),
            false,
            false,
            false,
        );
        togglable_columns.push(("size", tr!("Size").to_string(), size_col.clone()));
        view.append_column(&size_col);

        // Icon columns: link-only (Hoster/Availability are blank on package rows).
        let icon_col = |accessor: fn(&LinkGrabberRow) -> &gtk4::gio::Icon,
                        tooltip: fn(&LinkGrabberRow) -> &str,
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
                    let row = obj.borrow::<LinkGrabberRow>();
                    let text = tooltip(&row);
                    if text.is_empty() {
                        return false;
                    }
                    let tip_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                    let img = gtk4::Image::from_gicon(accessor(&row));
                    img.set_pixel_size(16);
                    tip_box.append(&img);
                    tip_box.append(&gtk4::Label::new(Some(text)));
                    ttip.set_custom(Some(&tip_box));
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
                    let row = obj.borrow::<LinkGrabberRow>();
                    image.set_from_gicon(accessor(&row));
                    image.set_visible(true);
                }
            });
            let col = gtk4::ColumnViewColumn::new(Some(title), Some(factory));
            col.set_fixed_width(min_width);
            col.set_resizable(false);
            col
        };

        let hoster_col = icon_col(
            |r| &r.host_icon,
            |r| &r.host_name,
            90,
            tr!("Hoster").as_ref(),
        );
        togglable_columns.push(("hoster", tr!("Hoster").to_string(), hoster_col.clone()));
        view.append_column(&hoster_col);

        let availability_col = icon_col(
            |r| &r.avail_icon,
            |r| &r.avail_tooltip,
            90,
            tr!("Availability").as_ref(),
        );
        togglable_columns.push((
            "availability",
            tr!("Availability").to_string(),
            availability_col.clone(),
        ));
        view.append_column(&availability_col);

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
            let saved = crate::config::get_bool_map("linkgrabber_columns");
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
                        crate::config::set_bool_entry("linkgrabber_columns", &key, c.is_active());
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

        // Sidebar
        let sidebar = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        sidebar.set_margin_start(6);
        sidebar.set_margin_end(6);
        sidebar.set_margin_top(6);
        sidebar.set_margin_bottom(6);

        let exceptions_frame = gtk4::Frame::new(Some("Exceptions"));
        let exceptions_list = gtk4::ListBox::new();
        exceptions_list.set_selection_mode(gtk4::SelectionMode::None);
        for item in &["Exception 1", "Exception 2"] {
            let row = gtk4::ListBoxRow::new();
            let label = gtk4::Label::new(Some(*item));
            label.set_halign(gtk4::Align::Start);
            label.set_margin_start(6);
            label.set_margin_end(6);
            row.set_child(Some(&label));
            exceptions_list.append(&row);
        }
        exceptions_frame.set_child(Some(&exceptions_list));
        sidebar.append(&exceptions_frame);

        let ext_frame = gtk4::Frame::new(Some("Extension filter"));
        let ext_list = gtk4::ListBox::new();
        ext_list.set_selection_mode(gtk4::SelectionMode::None);
        for item in &["*.mp4", "*.zip", "*.exe"] {
            let row = gtk4::ListBoxRow::new();
            let label = gtk4::Label::new(Some(*item));
            label.set_halign(gtk4::Align::Start);
            label.set_margin_start(6);
            label.set_margin_end(6);
            row.set_child(Some(&label));
            ext_list.append(&row);
        }
        ext_frame.set_child(Some(&ext_list));
        sidebar.append(&ext_frame);

        let hoster_frame = gtk4::Frame::new(Some("Hoster filter"));
        let hoster_list = gtk4::ListBox::new();
        hoster_list.set_selection_mode(gtk4::SelectionMode::None);
        for item in &["youtube.com", "mega.nz"] {
            let row = gtk4::ListBoxRow::new();
            let label = gtk4::Label::new(Some(*item));
            label.set_halign(gtk4::Align::Start);
            label.set_margin_start(6);
            label.set_margin_end(6);
            row.set_child(Some(&label));
            hoster_list.append(&row);
        }
        hoster_frame.set_child(Some(&hoster_list));
        sidebar.append(&hoster_frame);

        // Fixed-width right panel: 220px min and max, not user-resizable.
        // `set_size_request` is the authoritative floor here — GTK guarantees
        // natural size >= requested minimum, so this alone forces exactly
        // 220px for the current (narrow) content; `min/max-content-width` +
        // `propagate-natural-width` additionally cap it at 220px should the
        // filter lists ever contain wider content.
        let sidebar_scroll = gtk4::ScrolledWindow::builder()
            .child(&sidebar)
            .vexpand(true)
            .hexpand(false)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .min_content_width(220)
            .max_content_width(220)
            .propagate_natural_width(true)
            .build();
        sidebar_scroll.set_size_request(220, -1);

        let content_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        content_row.set_vexpand(true);
        content_row.set_hexpand(true);
        content_row.append(&table_overlay);
        content_row.append(&sidebar_scroll);

        page.append(&content_row);

        // Properties panel: shown for the selected link, hidden otherwise.
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
                let row = obj.borrow::<LinkGrabberRow>();
                properties.update(Some(crate::gui::properties_panel::PropertiesData {
                    link_id: row.uuid.parse().unwrap_or(0),
                    package_id: row.package_uuid,
                    file_name: row.file.clone(),
                    file_icon: row.file_icon.clone(),
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
            crate::gui::icon_key::ICON_LINKGRABBER,
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

        // Bottom bar split left/right
        let left_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        left_bar.set_valign(gtk4::Align::Center);

        // Add
        let add_btn = gtk4::Button::builder()
            .child(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_ADD)))
            .build();
        add_btn.set_has_frame(false);
        add_btn.set_tooltip_text(Some(tr!("Add links to linkgrabber").as_ref()));
        add_btn.set_size_request(24, 24);
        left_bar.append(&add_btn);

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
        add_arrow.set_child(Some(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_GO_DOWN))));
        add_arrow.set_popover(Some(&add_popover));
        add_arrow.set_size_request(12, 24);
        left_bar.append(&add_arrow);

        left_bar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

        // Clear
        let clear_btn = gtk4::Button::builder()
            .child(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_CLEAR)))
            .build();
        clear_btn.set_has_frame(false);
        clear_btn.set_tooltip_text(Some(tr!("Clear linkgrabber").as_ref()));
        clear_btn.set_size_request(24, 24);
        left_bar.append(&clear_btn);

        // Delete
        let delete_btn = gtk4::Button::builder()
            .child(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_TRASH)))
            .build();
        delete_btn.set_has_frame(false);
        delete_btn.set_tooltip_text(Some(tr!("Delete").as_ref()));
        delete_btn.set_size_request(24, 24);
        left_bar.append(&delete_btn);

        let delete_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        delete_popover_box.set_margin_start(8);
        delete_popover_box.set_margin_end(8);
        delete_popover_box.set_margin_top(8);
        delete_popover_box.set_margin_bottom(8);
        for label in [
            tr!("Delete disabled"),
            tr!("Delete offline"),
            tr!("Remove incomplete archives"),
            tr!("Clear filtered links"),
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
        delete_arrow.set_child(Some(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_GO_DOWN))));
        delete_arrow.set_popover(Some(&delete_popover));
        delete_arrow.set_size_request(12, 24);
        left_bar.append(&delete_arrow);

        left_bar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

        // Search
        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some(tr!("Filter").as_ref()));
        search.set_size_request(120, 24);
        left_bar.append(&search);

        // Add filtered
        let add_filtered = gtk4::Button::builder()
            .child(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_ADD)))
            .build();
        add_filtered.set_has_frame(false);
        add_filtered.set_tooltip_text(Some(tr!("Add filtered stuff").as_ref()));
        add_filtered.set_size_request(24, 24);
        left_bar.append(&add_filtered);

        // Right bar
        let right_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        right_bar.set_valign(gtk4::Align::Center);

        // Confirm
        let confirm_btn = gtk4::Button::builder()
            .label(tr!("Add to download list").as_str())
            .has_frame(false)
            .build();
        confirm_btn.set_size_request(120, 24);
        right_bar.append(&confirm_btn);

        let confirm_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        confirm_popover_box.set_margin_start(8);
        confirm_popover_box.set_margin_end(8);
        confirm_popover_box.set_margin_top(8);
        confirm_popover_box.set_margin_bottom(8);
        for label in [
            tr!("Add and start all"),
            tr!("Add and start selected"),
            tr!("Add all"),
            tr!("Add selected"),
        ] {
            let row = gtk4::Button::builder()
                .label(label.as_str())
                .has_frame(false)
                .halign(gtk4::Align::Start)
                .build();
            confirm_popover_box.append(&row);
        }
        let confirm_popover = gtk4::Popover::new();
        confirm_popover.set_child(Some(&confirm_popover_box));
        let confirm_arrow = gtk4::MenuButton::new();
        confirm_arrow.set_child(Some(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_GO_DOWN))));
        confirm_arrow.set_popover(Some(&confirm_popover));
        confirm_arrow.set_size_request(12, 24);
        right_bar.append(&confirm_arrow);

        right_bar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

        // Quick settings
        let settings_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        settings_popover_box.set_margin_start(8);
        settings_popover_box.set_margin_end(8);
        settings_popover_box.set_margin_top(8);
        settings_popover_box.set_margin_bottom(8);
        for label in [
            tr!("Add at top"),
            tr!("Auto confirm"),
            tr!("Auto start"),
            tr!("Link filter"),
            tr!("Properties"),
            tr!("Overview"),
            tr!("Sidebar"),
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
        settings_btn.set_child(Some(&gtk4::Image::from_gicon(&crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_SETTINGS))));
        settings_btn.set_popover(Some(&settings_popover));
        settings_btn.set_size_request(24, 24);
        right_bar.append(&settings_btn);

        let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);

        let bottom_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        bottom_bar.set_margin_start(6);
        bottom_bar.set_margin_end(6);
        bottom_bar.set_margin_top(3);
        bottom_bar.set_margin_bottom(3);
        bottom_bar.set_valign(gtk4::Align::Center);
        bottom_bar.set_size_request(-1, 24);
        bottom_bar.append(&left_bar);
        bottom_bar.append(&spacer);
        bottom_bar.append(&right_bar);

        page.append(&bottom_bar);

        Self {
            widget: page,
            store,
            child_stores,
            selection,
            view,
            overview,
            expanded_state,
        }
    }

    /// Starts the periodic refresh loop (every 2 s).
    ///
    /// Fetches collected links from JDownloader and updates the package/link
    /// tree. Triggers favicon downloads in the background for hosts not yet
    /// cached.
    pub fn start_refresh(&self, api: Arc<JdApi>) {
        let view = self.view.clone();
        let store = self.store.clone();
        let child_stores = self.child_stores.clone();
        let selection = self.selection.clone();
        let overview = self.overview.clone();
        let expanded_state = self.expanded_state.clone();
        let last_packages: Rc<RefCell<Vec<LinkGrabberPackageRow>>> =
            Rc::new(RefCell::new(Vec::new()));
        let last_links_by_package: Rc<RefCell<HashMap<i64, Vec<LinkGrabberRow>>>> =
            Rc::new(RefCell::new(HashMap::new()));
        let fetching: Arc<Mutex<std::collections::HashSet<String>>> =
            Arc::new(Mutex::new(std::collections::HashSet::new()));
        let variant_fetching: Arc<Mutex<std::collections::HashSet<String>>> =
            Arc::new(Mutex::new(std::collections::HashSet::new()));

        glib::source::timeout_add_local(Duration::from_secs(2), move || {
            let api = api.clone();
            let view = view.clone();
            let store = store.clone();
            let child_stores = child_stores.clone();
            let selection = selection.clone();
            let fetching = fetching.clone();
            let variant_fetching = variant_fetching.clone();
            let overview = overview.clone();
            let expanded_state = expanded_state.clone();
            let last_packages = last_packages.clone();
            let last_links_by_package = last_links_by_package.clone();

            let (tx, rx) = async_channel::bounded::<(Vec<Value>, Vec<Value>)>(1);
            let api_fetch = api.clone();
            std::thread::spawn(move || {
                if let Ok(data) = api_fetch.query_collector() {
                    let _ = tx.try_send(data);
                }
            });

            glib::MainContext::default().spawn_local(async move {
                if let Ok((packages_json, links_json)) = rx.recv().await {
                    update_overview_values(&overview, packages_json.len(), &links_json);
                    let new_packages: Vec<LinkGrabberPackageRow> =
                        packages_json.iter().map(package_row_from_json).collect();
                    let new_links: Vec<LinkGrabberRow> =
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

                    // Trigger variant icon downloads for keys not yet cached.
                    // Most variant icon keys are opaque, server-side
                    // composited descriptors (e.g. `"kc.<hash>"`) with no
                    // bundled/JD-theme PNG equivalent, so they must be
                    // fetched individually via /contentV2/getIcon.
                    for link_json in &links_json {
                        let key = link_json
                            .pointer("/variant/iconKey")
                            .or_else(|| link_json.pointer("/infoMap/variant/iconKey"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if key.is_empty()
                            || crate::gui::remote_icon::resolve(key).is_some()
                            || variant_fetching.lock().unwrap().contains(key)
                        {
                            continue;
                        }
                        variant_fetching.lock().unwrap().insert(key.to_string());
                        let key = key.to_string();
                        let api = api.clone();
                        let variant_fetching = variant_fetching.clone();
                        std::thread::spawn(move || {
                            if let Err(e) = crate::gui::remote_icon::fetch(&api, &key) {
                                log::warn!("variant icon for {}: {}", key, e);
                            }
                            variant_fetching.lock().unwrap().remove(&key);
                        });
                    }

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
                                packages.insert(obj.borrow::<LinkGrabberPackageRow>().uuid);
                            } else {
                                links.insert(obj.borrow::<LinkGrabberRow>().uuid.clone());
                            }
                        }
                        (packages, links)
                    };

                    let mut links_by_package: HashMap<i64, Vec<LinkGrabberRow>> = HashMap::new();
                    for row in new_links {
                        links_by_package.entry(row.package_uuid).or_default().push(row);
                    }

                    // sync_store/sync_store_keep_expanded rebuild rows via
                    // remove_all()+re-append when the list changes, which
                    // tears down the GtkListItem holding keyboard focus (if
                    // the user had clicked a row). GTK doesn't reassign focus
                    // on its own, so without this the Delete shortcut (wired
                    // on `view` in mod.rs) silently stops receiving events a
                    // few seconds after the row is touched.
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
                                && old.finished == new.finished
                        },
                        |_| {},
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
                                selected_packages
                                    .contains(&obj.borrow::<LinkGrabberPackageRow>().uuid)
                            } else {
                                selected_links.contains(&obj.borrow::<LinkGrabberRow>().uuid)
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
    /// them (== the order shown, since [`crate::gui::package_tree::sync_store`]
    /// never reorders items on its own). Used by the toolbar's move actions
    /// to compute a target package for `movePackages`'s `afterDestPackageId`.
    pub fn all_package_uuids(&self) -> Vec<i64> {
        (0..self.store.n_items())
            .filter_map(|pos| {
                self.store
                    .item(pos)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .map(|obj| obj.borrow::<LinkGrabberPackageRow>().uuid)
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
                let uuid = obj.borrow::<LinkGrabberPackageRow>().uuid;
                Some(uuid)
            })
            .collect()
    }
}

/// Walks the flattened tree and collects the uuid of every link that should
/// be removed for the current selection: individually selected link rows,
/// plus (mirroring JDownloader) every link belonging to a selected package
/// row, since deleting a package deletes all files inside it.
fn selected_uuids(
    selection: &gtk4::MultiSelection,
    child_stores: &Rc<RefCell<HashMap<i64, gio::ListStore>>>,
) -> std::collections::HashSet<String> {
    let bitset = selection.selection();
    let stores = child_stores.borrow();
    let mut uuids = std::collections::HashSet::new();
    for i in 0..bitset.size() {
        let pos = bitset.nth(i as u32);
        let Some(tree_row) = selection.item(pos).and_downcast::<gtk4::TreeListRow>() else {
            continue;
        };
        if tree_row.depth() == 0 {
            let Some(obj) = tree_row.item().and_downcast::<glib::BoxedAnyObject>() else {
                continue;
            };
            let package_uuid = obj.borrow::<LinkGrabberPackageRow>().uuid;
            if let Some(store) = stores.get(&package_uuid) {
                for cpos in 0..store.n_items() {
                    if let Some(child) = store.item(cpos).and_downcast::<glib::BoxedAnyObject>() {
                        uuids.insert(child.borrow::<LinkGrabberRow>().uuid.clone());
                    }
                }
            }
        } else if let Some(obj) = tree_row.item().and_downcast::<glib::BoxedAnyObject>() {
            uuids.insert(obj.borrow::<LinkGrabberRow>().uuid.clone());
        }
    }
    uuids
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
                if uuids.contains(&obj.borrow::<LinkGrabberRow>().uuid) {
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
            let uuid = obj.borrow::<LinkGrabberPackageRow>().uuid;
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

/// Config key for the "Don't show this again" checkbox on this dialog
/// (mirrors JDownloader's per-dialog dont-show-again registry, keyed there
/// by `GenericResetLinkgrabberRlyDialog.getDontShowAgainKey()`).
const SKIP_CONFIRM_KEY: &str = "skip_confirm_linkgrabber_remove";

/// Shows JDownloader's "Are you sure?" removal dialog for the link grabber
/// (mirrors `GenericResetLinkgrabberRlyDialog`). Collected links haven't
/// been downloaded yet, so unlike the downloads list there is no
/// recycle/delete-on-disk choice. Always shown (even for a single link) as
/// JDownloader does, unless the user previously ticked "Don't show this
/// again". Calls `on_confirm` if the user confirms (or if the dialog is
/// currently skipped).
///
/// Uses a plain `gtk4::Window` rather than `adw::AlertDialog` so the "Don't
/// show this again" checkbox can sit on the same row as the Continue/Cancel
/// buttons, matching JDownloader's layout (`AlertDialog`'s response row is
/// fixed and can't host extra widgets).
fn confirm_remove<W: IsA<gtk4::Widget> + Clone + 'static>(
    parent: &W,
    link_count: usize,
    links_left: usize,
    on_confirm: impl FnOnce() + 'static,
) {
    if crate::config::get_bool_map("dialog_prefs")
        .get(SKIP_CONFIRM_KEY)
        .copied()
        .unwrap_or(false)
    {
        on_confirm();
        return;
    }

    let Some(parent_window) = parent.root().and_then(|r| r.downcast::<gtk4::Window>().ok())
    else {
        on_confirm();
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
        crate::gui::icon_key::ICON_BOTTY_ROBOT_DEL,
    ));
    robot.set_pixel_size(100);
    robot.set_valign(gtk4::Align::Start);
    content.append(&robot);

    let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    text_box.set_hexpand(true);

    let intro_label = gtk4::Label::new(Some(&format!(
        "{}\n{}",
        tr!("Do you really want to perform this clean up action:"),
        tr!("Delete Selected Links?")
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

    let task_text = format!(
        "{} — {}",
        tr!("Delete selected link" | "Delete selected links" % link_count),
        tr!("{n} link remaining" | "{n} links remaining" % links_left),
    );
    let task_label = gtk4::Label::new(Some(&task_text));
    task_label.set_halign(gtk4::Align::Start);
    task_label.set_xalign(0.0);
    task_label.set_wrap(true);
    task_label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
    text_box.append(&task_label);

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
        if let Some(f) = on_confirm.borrow_mut().take() {
            f();
        }
        dialog_c.close();
    });

    dialog.present();
    continue_btn.grab_focus();
}

/// Removes the selected links from the link collector, mirroring
/// JDownloader's "Remove" action: prompts for confirmation (as JDownloader
/// always does, regardless of selection size) before removing the links.
pub fn remove_selected(
    api: &Arc<JdApi>,
    selection: &gtk4::MultiSelection,
    store: &gio::ListStore,
    child_stores: &Rc<RefCell<HashMap<i64, gio::ListStore>>>,
    view: &gtk4::ColumnView,
) {
    let uuids = selected_uuids(selection, child_stores);
    if uuids.is_empty() {
        return;
    }
    let link_count = uuids.len();
    let links_left: usize = child_stores
        .borrow()
        .values()
        .map(|s| s.n_items() as usize)
        .sum::<usize>()
        - link_count;

    // Lowest flattened-tree position among the rows about to be removed:
    // once they're gone, the row that slides up into this position is the
    // one immediately below the deleted selection.
    let focus_pos = selection.selection().minimum();

    let api = api.clone();
    let store = store.clone();
    let child_stores = child_stores.clone();
    let selection = selection.clone();
    let view = view.clone();
    confirm_remove(&view.clone(), link_count, links_left, move || {
        let ids: Vec<i64> = uuids.iter().filter_map(|u| u.parse().ok()).collect();
        crate::gui::spawn::api_call(
            move || {
                let _ = api.remove_linkgrabber_links(&ids);
            },
            move |_| {
                remove_rows_by_uuid(&child_stores, &uuids);
                prune_empty_packages(&store, &child_stores);
                super::downloads_panel::select_and_focus_row(&view, &selection, focus_pos);
            },
        );
    });
}

/// Recomputes every Overview stat from the flat list of collected links
/// (mirrors JDownloader's `AggregatedCrawlerNumbers`), and pushes the
/// results into `overview`.
fn update_overview_values(overview: &crate::gui::overview_panel::OverviewPanel, packages_len: usize, links_json: &[Value]) {
    let mut bytes_total: i64 = 0;
    let mut online = 0i64;
    let mut offline = 0i64;
    let mut unknown = 0i64;
    let mut hosters: std::collections::HashSet<String> = std::collections::HashSet::new();
    for link in links_json {
        bytes_total += super::downloads_panel::get_num_at(link, "bytesTotal").unwrap_or(0);
        let host = super::downloads_panel::get_str_at(link, "host");
        if !host.is_empty() {
            hosters.insert(host);
        }
        match super::downloads_panel::get_str_at(link, "availability").as_str() {
            "ONLINE" | "Online" => online += 1,
            "OFFLINE" | "Offline" => offline += 1,
            _ => unknown += 1,
        }
    }

    overview.set_value(OVERVIEW_PACKAGES, &packages_len.to_string());
    overview.set_value(OVERVIEW_LINKS, &links_json.len().to_string());
    overview.set_value(OVERVIEW_SIZE, &super::downloads_panel::format_size_jd(bytes_total));
    overview.set_value(OVERVIEW_ONLINE, &online.to_string());
    overview.set_value(OVERVIEW_HOSTER, &hosters.len().to_string());
    overview.set_value(OVERVIEW_OFFLINE, &offline.to_string());
    overview.set_value(OVERVIEW_UNKNOWN, &unknown.to_string());
}

fn package_row_from_json(pkg: &Value) -> LinkGrabberPackageRow {
    let get_str = |key: &str| -> String {
        pkg.pointer(&format!("/{}", key))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let get_num = |key: &str| -> Option<i64> { pkg.pointer(&format!("/{}", key)).and_then(Value::as_i64) };
    let get_bool = |key: &str| -> bool {
        pkg.pointer(&format!("/{}", key))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let bytes_total = get_num("bytesTotal").unwrap_or(0);
    let child_count = get_num("childCount").unwrap_or(0);
    LinkGrabberPackageRow {
        uuid: get_num("uuid").unwrap_or(0),
        name: get_str("name"),
        save_to: get_str("saveTo"),
        child_count,
        bytes_total,
        size_text: super::downloads_panel::format_size_jd(bytes_total),
        enabled: get_bool("enabled"),
        finished: get_bool("finished"),
        comment: get_str("comment"),
    }
}

fn row_from_json(link: &Value) -> LinkGrabberRow {
    let get_str = |key: &str| -> String {
        link.pointer(&format!("/{}", key))
            .or_else(|| link.pointer(&format!("/infoMap/{}", key)))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let get_num = |key: &str| -> Option<i64> {
        link.pointer(&format!("/{}", key))
            .or_else(|| link.pointer(&format!("/infoMap/{}", key)))
            .and_then(Value::as_i64)
    };
    let name = link
        .pointer("/name")
        .or_else(|| link.pointer("/infoMap/name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let host = get_str("host");
    let save_to = get_str("saveTo");
    let size = get_num("bytesTotal")
        .or_else(|| get_num("size"))
        .map(super::downloads_panel::format_size_jd)
        .unwrap_or_default();
    let availability = get_str("availability");
    let uuid = get_num("uuid")
        .map(|n| n.to_string())
        .unwrap_or_default();
    let package_uuid = get_num("packageUUID").unwrap_or(0);
    let package_name = get_str("packageName");
    let url = get_str("url");
    let priority = {
        let p = get_str("priority");
        if p.is_empty() { "DEFAULT".to_string() } else { p }
    };
    let comment = get_str("comment");
    let variant_id = get_str("variant/id");
    let variant_name = get_str("variant/name");
    let variant_icon_key = get_str("variant/iconKey");
    let variant_icon_resolved = crate::gui::remote_icon::resolve(&variant_icon_key);
    let has_variant_icon = variant_icon_resolved.is_some();
    let variant_icon =
        variant_icon_resolved.unwrap_or_else(|| crate::gui::jd_icon::resolve(&variant_icon_key));
    let file_icon = crate::gui::jd_icon::resolve(super::downloads_panel::file_type_icon_key(&name));
    let has_host_icon = !host.is_empty() && crate::gui::jd_icon::resolve_path(&host).is_some();
    let host_icon = crate::gui::jd_icon::resolve_or(&host, crate::gui::icon_key::ICON_FILE);
    let (avail_icon, avail_tooltip) = match availability.as_str() {
        "ONLINE" | "Online" => (
            crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_OK),
            tr!("Online").to_string(),
        ),
        "OFFLINE" | "Offline" => (
            crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_ERROR),
            tr!("Offline").to_string(),
        ),
        "TEMP_UNKNOWN" | "TEMPORARILY_UNAVAILABLE" | "Temporarily Unknown" => (
            crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_QUESTION),
            tr!("Temporarily Unknown").to_string(),
        ),
        _ => (
            crate::gui::jd_icon::resolve(crate::gui::icon_key::ICON_QUESTION),
            tr!("Unknown").to_string(),
        ),
    };
    LinkGrabberRow {
        file: name,
        file_icon,
        variant: variant_name,
        variant_id,
        variant_icon,
        has_variant_icon,
        save_to,
        size,
        host_icon,
        host_name: host,
        has_host_icon,
        avail_icon,
        avail_tooltip,
        uuid,
        package_uuid,
        package_name,
        url,
        priority,
        comment,
    }
}
