//! Shared scaffolding for the package/link tree views used by the Downloads
//! and Link Grabber panels: both show a `GtkColumnView` over a
//! `GtkTreeListModel` of collapsible package rows, each with its own child
//! store of link rows, and persist each package's expand/collapse state
//! across refreshes. This module holds the parts of that plumbing which
//! don't depend on either panel's specific row types.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;

/// Returns the item at `list_item`'s position unwrapped from its
/// `TreeListRow`, along with the row itself (for depth/expander access).
pub(crate) fn tree_item(list_item: &gtk4::ListItem) -> Option<(gtk4::TreeListRow, glib::BoxedAnyObject)> {
    let tree_row = list_item.item().and_downcast::<gtk4::TreeListRow>()?;
    let obj = tree_row.item()?.downcast::<glib::BoxedAnyObject>().ok()?;
    Some((tree_row, obj))
}

/// Resolves the open/closed package icon for a given expand state, mirroring
/// JDownloader's own package row icon.
pub(crate) fn package_icon(expanded: bool) -> gio::Icon {
    crate::gui::jd_icon::resolve(if expanded {
        crate::gui::icon_key::ICON_PACKAGE_OPEN
    } else {
        crate::gui::icon_key::ICON_PACKAGE_CLOSED
    })
}

/// Diffs `new_items` against `last_items` (by `PartialEq`), patching `store`
/// in place item-by-item when `key(item)` is in the same order as before
/// (the common case: just field changes), or fully replacing the store's
/// contents when items were added, removed, or reordered. Never touches rows
/// that didn't change, so their selection state is left alone.
pub(crate) fn sync_store<T, K: PartialEq>(
    store: &gio::ListStore,
    last_items: &mut Vec<T>,
    new_items: Vec<T>,
    key: impl Fn(&T) -> K,
) where
    T: PartialEq + Clone + 'static,
{
    let same_order = last_items.len() == new_items.len()
        && last_items.iter().zip(&new_items).all(|(o, n)| key(o) == key(n));
    if same_order {
        for (pos, (old_item, new_item)) in last_items.iter().zip(&new_items).enumerate() {
            if old_item != new_item {
                store.splice(pos as u32, 1, &[glib::BoxedAnyObject::new(new_item.clone())]);
            }
        }
    } else {
        store.remove_all();
        for item in &new_items {
            store.append(&glib::BoxedAnyObject::new(item.clone()));
        }
    }
    *last_items = new_items;
}

/// Like [`sync_store`], but for a *package*-level top store: when a changed
/// item's `keep_in_place` (given the old and new item) returns true, its
/// `BoxedAnyObject` is mutated in place (`glib::BoxedAnyObject::replace`)
/// instead of being spliced in as a new one.
///
/// GTK's `GtkTreeListModel` has no lighter-weight "this item's value
/// changed but it's still the same node" signal — any splice at a position
/// (even replacing an item with an equal-`key` one) makes it discard that
/// row's expanded state and cached child model outright. For a package
/// that's currently expanded, that meant every refresh which changed so
/// much as its size/speed/status text (i.e. constantly, while downloading)
/// silently collapsed it again. Mutating in place instead means the
/// package's own row doesn't get a fresh `connect_bind` on every tick while
/// expanded (its children, which live in their own store, keep updating
/// live regardless) — `on_kept_in_place` is the caller's hook to repaint
/// that row's already-bound widgets some other way (see
/// [`PackageLiveRefresh`]), so the row doesn't go visually stale for as long
/// as it stays expanded.
///
/// `keep_in_place` gets both the old and new item (not just "is this
/// expanded?") so callers can force a real splice on a significant
/// transition (e.g. a package finishing) even while expanded: a genuine
/// rebind is the one path guaranteed to repaint correctly, so it's worth
/// paying for occasionally, on transitions that matter, rather than always
/// trusting every column's own live-refresh registry to have a widget
/// registered for this exact uuid at this exact moment.
pub(crate) fn sync_store_keep_expanded<T, K: PartialEq>(
    store: &gio::ListStore,
    last_items: &mut Vec<T>,
    new_items: Vec<T>,
    key: impl Fn(&T) -> K,
    keep_in_place: impl Fn(&T, &T) -> bool,
    on_kept_in_place: impl Fn(&T),
) where
    T: PartialEq + Clone + 'static,
{
    let same_order = last_items.len() == new_items.len()
        && last_items.iter().zip(&new_items).all(|(o, n)| key(o) == key(n));
    if same_order {
        for (pos, (old_item, new_item)) in last_items.iter().zip(&new_items).enumerate() {
            if old_item != new_item {
                if keep_in_place(old_item, new_item) {
                    if let Some(obj) = store.item(pos as u32).and_downcast::<glib::BoxedAnyObject>() {
                        obj.replace(new_item.clone());
                        on_kept_in_place(new_item);
                        continue;
                    }
                }
                store.splice(pos as u32, 1, &[glib::BoxedAnyObject::new(new_item.clone())]);
            }
        }
    } else {
        store.remove_all();
        for item in &new_items {
            store.append(&glib::BoxedAnyObject::new(item.clone()));
        }
    }
    *last_items = new_items;
}

/// Per-column registry of "how to repaint this package row's widget"
/// closures, keyed by package uuid, for a package row type `P`. Populated by
/// a column's `connect_bind` (and cleared by its `connect_unbind`) for
/// whichever package row is currently bound to that column's widget, so the
/// refresh loop can call straight into it — via `sync_store_keep_expanded`'s
/// `on_kept_in_place` hook — to repaint an expanded package's row without a
/// real GTK rebind.
pub(crate) type PackageLiveRefresh<P> = Rc<RefCell<HashMap<i64, Box<dyn Fn(&P)>>>>;

/// The tree scaffolding shared by the Downloads and Link Grabber panels:
/// a top-level store of package rows, each package's child store of link
/// rows (lazily created, keyed by package uuid), the `MultiSelection` (over
/// a `TreeListModel`) built from them, and the persisted expand/collapse
/// state consulted by [`sync_store_keep_expanded`] and by the name column
/// built by [`build_name_column`].
pub(crate) struct PackageTree {
    pub store: gio::ListStore,
    pub child_stores: Rc<RefCell<HashMap<i64, gio::ListStore>>>,
    pub expanded_state: Rc<RefCell<HashMap<i64, bool>>>,
    pub selection: gtk4::MultiSelection,
}

impl PackageTree {
    /// `config_key` is the `crate::config` expand-state namespace
    /// (`"downloads"` / `"linkgrabber"`); `uuid_of` extracts a package row's
    /// uuid from its `BoxedAnyObject` — the `TreeListModel`'s child-model
    /// lookup needs it to key `child_stores`, but GTK's create-fn type-erases
    /// the item to `glib::Object`, so the concrete package row type `P` has
    /// to be threaded through as a generic parameter here.
    pub(crate) fn build<P: 'static>(config_key: &'static str, uuid_of: impl Fn(&P) -> i64 + 'static) -> Self {
        let store = gio::ListStore::builder()
            .item_type(glib::BoxedAnyObject::static_type())
            .build();

        let child_stores: Rc<RefCell<HashMap<i64, gio::ListStore>>> =
            Rc::new(RefCell::new(HashMap::new()));

        // Remembers each package's expand/collapse state across refreshes
        // (packages start collapsed, like JDownloader).
        let expanded_state: Rc<RefCell<HashMap<i64, bool>>> =
            Rc::new(RefCell::new(crate::config::get_expanded_map(config_key)));

        let tree_model = {
            let child_stores = child_stores.clone();
            gtk4::TreeListModel::new(store.clone(), false, false, move |item| {
                let obj = item.downcast_ref::<glib::BoxedAnyObject>()?;
                let borrowed = obj.try_borrow::<P>().ok()?;
                let uuid = uuid_of(&borrowed);
                let child = child_stores
                    .borrow_mut()
                    .entry(uuid)
                    .or_insert_with(|| {
                        gio::ListStore::builder()
                            .item_type(glib::BoxedAnyObject::static_type())
                            .build()
                    })
                    .clone();
                Some(child.upcast::<gio::ListModel>())
            })
        };

        let selection = gtk4::MultiSelection::new(Some(tree_model));

        Self {
            store,
            child_stores,
            expanded_state,
            selection,
        }
    }
}

/// A child row's icon/label accessors, as passed to [`build_name_column`].
pub(crate) type ChildAccessors<C> = (fn(&C) -> &gio::Icon, fn(&C) -> &str);

/// Builds the tree's leading name column: a `GtkTreeExpander` (indentation +
/// expand triangle) wrapping an icon + label, showing `package_icon`/
/// `package_name` for package rows (depth 0, row type `P`) and `child_icon`/
/// `child_label` for child rows (depth 1, row type `C`).
///
/// Also owns the expand/collapse persistence dance: restoring each package's
/// saved expand state (deferred to idle, since this bind can itself be
/// running mid-update — see the inline comment) and saving it back (to both
/// `expanded_state` and `crate::config`) whenever the user toggles a row.
pub(crate) fn build_name_column<P: 'static, C: 'static>(
    title: &str,
    min_width: i32,
    config_key: &'static str,
    expanded_state: Rc<RefCell<HashMap<i64, bool>>>,
    package_uuid: fn(&P) -> i64,
    package_name: fn(&P) -> &str,
    child_accessors: ChildAccessors<C>,
) -> gtk4::ColumnViewColumn {
    let (child_icon, child_label) = child_accessors;
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(move |_, list_item| {
        let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        hbox.set_halign(gtk4::Align::Fill);
        hbox.set_hexpand(true);
        let icon = gtk4::Image::new();
        icon.set_pixel_size(16);
        icon.set_valign(gtk4::Align::Center);
        let label = gtk4::Label::new(None);
        label.set_halign(gtk4::Align::Fill);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        label.set_has_tooltip(true);
        hbox.append(&icon);
        hbox.append(&label);

        let expander = gtk4::TreeExpander::new();
        expander.set_hexpand(true);
        expander.set_child(Some(&hbox));

        list_item.set_child(Some(&expander));
    });
    factory.connect_bind({
        let expanded_state = expanded_state.clone();
        move |_, list_item| {
            let expanded_state = expanded_state.clone();
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let Some(tree_row) = list_item.item().and_downcast::<gtk4::TreeListRow>() else {
                return;
            };
            let Some(obj) = tree_row
                .item()
                .and_then(|i| i.downcast::<glib::BoxedAnyObject>().ok())
            else {
                return;
            };
            let Some(expander) = list_item.child().and_downcast::<gtk4::TreeExpander>() else {
                return;
            };
            expander.set_list_row(Some(&tree_row));
            let Some(hbox) = expander.child().and_downcast::<gtk4::Box>() else {
                return;
            };
            let Some(icon) = hbox.first_child().and_downcast::<gtk4::Image>() else {
                return;
            };
            let Some(label) = icon.next_sibling().and_downcast::<gtk4::Label>() else {
                return;
            };
            if tree_row.depth() == 0 {
                let uuid = package_uuid(&obj.borrow::<P>());
                let desired = expanded_state.borrow().get(&uuid).copied().unwrap_or(false);
                if tree_row.is_expanded() != desired {
                    // Deferred to idle: this bind is itself running as GTK
                    // processes the items-changed that just recreated this
                    // row (e.g. a package's fields changed, so `sync_store`
                    // replaced its list item). Expanding synchronously here
                    // re-enters the tree model mid-update and the row can
                    // end up visually collapsed anyway.
                    let deferred_row = tree_row.clone();
                    glib::source::idle_add_local_once(move || {
                        if deferred_row.is_expanded() != desired {
                            deferred_row.set_expanded(desired);
                        }
                    });
                }
                icon.set_from_gicon(&package_icon(tree_row.is_expanded()));
                tree_row.connect_expanded_notify({
                    let expanded_state = expanded_state.clone();
                    let icon = icon.clone();
                    move |row| {
                        expanded_state.borrow_mut().insert(uuid, row.is_expanded());
                        crate::config::set_expanded(config_key, uuid, row.is_expanded());
                        icon.set_from_gicon(&package_icon(row.is_expanded()));
                    }
                });
                let pkg = obj.borrow::<P>();
                let name = package_name(&pkg);
                label.set_text(name);
                label.set_tooltip_text(Some(name));
            } else {
                let row = obj.borrow::<C>();
                icon.set_from_gicon(child_icon(&row));
                label.set_text(child_label(&row));
                label.set_tooltip_text(Some(child_label(&row)));
            }
        }
    });
    let col = gtk4::ColumnViewColumn::new(Some(title), Some(factory));
    col.set_fixed_width(min_width);
    col.set_resizable(false);
    col.set_expand(true);
    col
}
