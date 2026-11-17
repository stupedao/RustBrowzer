//! Linux webview layout.
//!
//! Tauri's `unstable` multi-webview API creates child webviews on Linux with
//! `WebviewKind::WindowChild`, which packs each of them into the window's
//! default `GtkBox`. A box stacks its children and ignores their position, so
//! the window's own webview and every tab webview end up splitting the window
//! vertically. `Webview::set_position`/`set_size` are no-ops for them because
//! wry only applies bounds when the webview sits in a `GtkFixed`.
//!
//! The fix is a `GtkFixed` that holds every webview of the window. It asks for
//! no room (so the window can still shrink) and has its own `GdkWindow` (so the
//! webviews are clipped at its edges). wry does not see the container as a
//! fixed parent (webviews are created before we move them), so the container
//! also applies the bounds itself.
//!
//! This mirrors the direction of tauri#16507, which is not released yet.

use gtk::glib;
use gtk::glib::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;

thread_local! {
    /// The container of the main window. GTK objects are main-thread only.
    static CONTAINER: RefCell<Option<WebviewContainer>> = RefCell::new(None);
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct WebviewContainer {
        /// The window's own webview (the browser UI), kept at the full room.
        pub main: RefCell<Option<gtk::Widget>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WebviewContainer {
        const NAME: &'static str = "RustBrowzerWebviewContainer";
        type Type = super::WebviewContainer;
        type ParentType = gtk::Fixed;
    }

    impl ObjectImpl for WebviewContainer {
        fn constructed(&self) {
            self.parent_constructed();
            // Clip the webviews (and their input) at the container edges.
            self.obj().set_has_window(true);
        }
    }

    impl WidgetImpl for WebviewContainer {
        // A GtkFixed asks for the room of its children. That would stop the
        // window from shrinking below its webviews, so ask for nothing.
        fn preferred_width(&self) -> (i32, i32) {
            (0, 0)
        }

        fn preferred_height(&self) -> (i32, i32) {
            (0, 0)
        }

        fn size_allocate(&self, allocation: &gtk::Allocation) {
            self.parent_size_allocate(allocation);

            if allocation.width() <= 1 || allocation.height() <= 1 {
                return;
            }

            let Some(main) = self.main.borrow().clone() else {
                return;
            };

            let (width, height) = (allocation.width(), allocation.height());
            if main.allocated_width() != width || main.allocated_height() != height {
                main.set_size_request(width, height);
            }
            // The requisition that GtkFixed used above can lag one layout pass
            // behind, so apply the room directly as well.
            main.size_allocate(&gtk::Allocation::new(0, 0, width, height));
        }
    }

    impl ContainerImpl for WebviewContainer {}
    impl FixedImpl for WebviewContainer {}
}

glib::wrapper! {
    /// Holds every webview of the main window at its bounds.
    pub struct WebviewContainer(ObjectSubclass<imp::WebviewContainer>)
        @extends gtk::Fixed, gtk::Container, gtk::Widget;
}

impl WebviewContainer {
    fn new() -> Self {
        glib::Object::new()
    }

    /// Moves the window's own webview into the container.
    fn set_main(&self, widget: &gtk::Widget) {
        if let Some(parent) = widget
            .parent()
            .and_then(|p| p.dynamic_cast::<gtk::Container>().ok())
        {
            parent.remove(widget);
        }
        self.put(widget, 0, 0);

        let (width, height) = (self.allocated_width(), self.allocated_height());
        if width > 1 && height > 1 {
            widget.set_size_request(width, height);
        }
        *self.imp().main.borrow_mut() = Some(widget.clone());
        widget.show_all();
    }
}

/// Replaces the window content with the webview container. Call on the GTK
/// main thread, after the window and its own webview exist.
pub fn install(vbox: &gtk::Box) {
    let main = vbox
        .children()
        .into_iter()
        .find(|w| w.type_().name() == "WebKitWebView");

    let container = WebviewContainer::new();
    vbox.pack_start(&container, true, true, 0);
    container.show();

    if let Some(main) = main {
        container.set_main(&main);
    }

    CONTAINER.with(|slot| *slot.borrow_mut() = Some(container));
}

/// Moves a tab webview into the container at the viewport bounds (logical
/// pixels). Safe to call repeatedly while a tab webview is resized.
pub fn place<W: IsA<gtk::Widget>>(widget: &W, x: f64, y: f64, width: f64, height: f64) {
    let widget = widget.upcast_ref::<gtk::Widget>();
    let x = x.round() as i32;
    let y = y.round() as i32;
    let width = (width.round() as i32).max(1);
    let height = (height.round() as i32).max(1);

    CONTAINER.with(|slot| {
        let slot = slot.borrow();
        let Some(container) = slot.as_ref() else {
            return;
        };

        let in_container = widget
            .parent()
            .map(|parent| parent.as_ptr() == container.upcast_ref::<gtk::Widget>().as_ptr())
            .unwrap_or(false);

        if in_container {
            container.move_(widget, x, y);
        } else {
            if let Some(parent) = widget
                .parent()
                .and_then(|p| p.dynamic_cast::<gtk::Container>().ok())
            {
                parent.remove(widget);
            }
            container.put(widget, x, y);
        }

        if widget.allocated_width() != width || widget.allocated_height() != height {
            widget.set_size_request(width, height);
        }
        widget.show_all();
    });
}

/// Install window-level keyboard accelerators (Ctrl+T / Ctrl+W / Ctrl+L /
/// Ctrl+Shift+T / Ctrl+Tab …) that keep working even while a web page holds
/// keyboard focus. GTK routes accelerators before the focus widget.
pub fn install_accelerators(window: &tauri::WebviewWindow, app: tauri::AppHandle) {
    use gtk::prelude::GtkWindowExt;

    let Ok(gtk_window) = window.gtk_window() else {
        return;
    };
    let accel = gtk::AccelGroup::new();
    gtk_window.add_accel_group(&accel);

    const CTRL: gtk::gdk::ModifierType = gtk::gdk::ModifierType::CONTROL_MASK;
    const CTRL_SHIFT: gtk::gdk::ModifierType =
        gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK;

    // (key, mods, action) — action names are understood by the UI webview.
    const BINDINGS: &[(char, gtk::gdk::ModifierType, &str)] = &[
        ('t', CTRL, "accel-new-tab"),
        ('w', CTRL, "accel-close-tab"),
        ('l', CTRL, "accel-focus-omnibox"),
        ('t', CTRL_SHIFT, "accel-reopen-tab"),
        ('r', CTRL, "accel-reload"),
        ('b', CTRL, "accel-toggle-bookmarks"),
        ('h', CTRL, "accel-toggle-history"),
        ('j', CTRL, "accel-toggle-downloads"),
    ];

    for &(ch, mods, action) in BINDINGS {
        let key = ch.to_ascii_lowercase() as u32;
        let app = app.clone();
        let action = action.to_string();
        accel.connect_accel_group(
            key,
            mods,
            gtk::AccelFlags::empty(),
            move |_group, _obj, _key, _mods| {
                use tauri::Emitter;
                // Route the shortcut into the UI webview, which owns the
                // keyboard handling logic. Content webviews never see it.
                let _ = app.emit_to(
                    crate::renderer::UI_LABEL,
                    "accelerator",
                    action.clone(),
                );
                true // stop propagation; the web page must not also act on it
            },
        );
    }
}
