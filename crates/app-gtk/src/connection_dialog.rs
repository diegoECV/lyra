use crate::connection_manager::{ConnectionInfo, ConnectionManager};
use libadwaita::glib;
use libadwaita::gtk::prelude::*;
use libadwaita::gtk::{Align, Box as GtkBox, Button, Entry, Label, Orientation};
use libadwaita::prelude::*;
use libadwaita::{HeaderBar, ToolbarView, Window as AdwWindow, WindowTitle};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

pub fn show_connection_dialog<F>(
    parent: &libadwaita::ApplicationWindow,
    mgr_cell: Rc<RefCell<Option<ConnectionManager>>>,
    on_connected: F,
) where
    F: Fn(ConnectionInfo) + 'static,
{
    let dialog = AdwWindow::builder()
        .transient_for(parent)
        .modal(true)
        .title("Nueva Conexión")
        .default_width(520)
        .default_height(480)
        .build();

    let vbox = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_start(20)
        .margin_end(20)
        .margin_top(16)
        .margin_bottom(20)
        .build();

    // 1. Selector visual de motor
    let engine_title = Label::builder()
        .label("Selecciona el motor de base de datos:")
        .halign(Align::Start)
        .css_classes(["bold-title"])
        .build();
    vbox.append(&engine_title);

    let engine_cards_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .homogeneous(true)
        .build();

    let sqlite_btn = Button::builder()
        .label("🗄️\nSQLite")
        .css_classes(["engine-card", "engine-card-selected"])
        .build();

    let pg_btn = Button::builder()
        .label("🐘\nPostgreSQL")
        .css_classes(["engine-card"])
        .build();

    let mongo_btn = Button::builder()
        .label("🍃\nMongoDB")
        .css_classes(["engine-card"])
        .build();

    let redis_btn = Button::builder()
        .label("⚡\nRedis")
        .css_classes(["engine-card"])
        .build();

    engine_cards_box.append(&sqlite_btn);
    engine_cards_box.append(&pg_btn);
    engine_cards_box.append(&mongo_btn);
    engine_cards_box.append(&redis_btn);
    vbox.append(&engine_cards_box);

    // 2. Campos de entrada
    let form_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(8)
        .build();

    let name_lbl = Label::builder()
        .label("Nombre de la conexión:")
        .halign(Align::Start)
        .css_classes(["subtle-label"])
        .build();

    let name_entry = Entry::builder()
        .text("Base de Datos SQLite")
        .placeholder_text("Ej. Producción, Datos Locales...")
        .build();

    let url_lbl = Label::builder()
        .label("Cadena de conexión (URL):")
        .halign(Align::Start)
        .css_classes(["subtle-label"])
        .build();

    let url_entry = Entry::builder()
        .text("sqlite::memory:")
        .placeholder_text("sqlite::memory: o sqlite:///ruta/archivo.db")
        .build();

    form_box.append(&name_lbl);
    form_box.append(&name_entry);
    form_box.append(&url_lbl);
    form_box.append(&url_entry);
    vbox.append(&form_box);

    // Cambiar plantillas al presionar tarjetas de motor
    let buttons = [
        sqlite_btn.clone(),
        pg_btn.clone(),
        mongo_btn.clone(),
        redis_btn.clone(),
    ];
    let clear_selection = {
        let buttons = buttons.clone();
        move || {
            for b in &buttons {
                b.remove_css_class("engine-card-selected");
            }
        }
    };

    {
        let clear_sel = clear_selection.clone();
        let btn_c = sqlite_btn.clone();
        let name_e = name_entry.clone();
        let url_e = url_entry.clone();
        sqlite_btn.connect_clicked(move |_| {
            clear_sel();
            btn_c.add_css_class("engine-card-selected");
            name_e.set_text("Base de Datos SQLite");
            url_e.set_text("sqlite::memory:");
            url_e.set_placeholder_text(Some("sqlite::memory: o sqlite:///ruta/archivo.db"));
        });
    }

    {
        let clear_sel = clear_selection.clone();
        let btn_c = pg_btn.clone();
        let name_e = name_entry.clone();
        let url_e = url_entry.clone();
        pg_btn.connect_clicked(move |_| {
            clear_sel();
            btn_c.add_css_class("engine-card-selected");
            name_e.set_text("PostgreSQL Servidor");
            url_e.set_text("postgres://postgres:password@localhost:5432/postgres");
            url_e.set_placeholder_text(Some("postgres://user:pass@host:port/database"));
        });
    }

    {
        let clear_sel = clear_selection.clone();
        let btn_c = mongo_btn.clone();
        let name_e = name_entry.clone();
        let url_e = url_entry.clone();
        mongo_btn.connect_clicked(move |_| {
            clear_sel();
            btn_c.add_css_class("engine-card-selected");
            name_e.set_text("MongoDB Colecciones");
            url_e.set_text("mongodb://localhost:27017/admin");
            url_e.set_placeholder_text(Some("mongodb://localhost:27017/database"));
        });
    }

    {
        let clear_sel = clear_selection;
        let btn_c = redis_btn.clone();
        let name_e = name_entry.clone();
        let url_e = url_entry.clone();
        redis_btn.connect_clicked(move |_| {
            clear_sel();
            btn_c.add_css_class("engine-card-selected");
            name_e.set_text("Redis Caché");
            url_e.set_text("redis://127.0.0.1:6379");
            url_e.set_placeholder_text(Some("redis://127.0.0.1:6379"));
        });
    }

    // 3. Estado de prueba
    let test_status_lbl = Label::builder()
        .label("")
        .halign(Align::Start)
        .hexpand(true)
        .build();
    vbox.append(&test_status_lbl);

    // 4. Botones de acción inferiores
    let actions_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(10)
        .build();

    let test_btn = Button::builder().label("⚡ Probar Conexión").build();

    let cancel_btn = Button::builder().label("Cancelar").build();

    let connect_btn = Button::builder()
        .label("Conectar")
        .css_classes(["suggested-action", "pill"])
        .build();

    actions_box.append(&test_btn);
    actions_box.append(&test_status_lbl);
    actions_box.append(&cancel_btn);
    actions_box.append(&connect_btn);
    vbox.append(&actions_box);

    let header = HeaderBar::builder()
        .title_widget(&WindowTitle::new("Añadir Base de Datos", "SQL & NoSQL"))
        .show_end_title_buttons(true)
        .build();

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&vbox));
    dialog.set_content(Some(&toolbar));

    // Cancelar
    let dlg_c = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dlg_c.close();
    });

    // Probar conexión
    let url_e_test = url_entry.clone();
    let test_lbl = test_status_lbl.clone();
    let test_btn_c = test_btn.clone();
    test_btn.connect_clicked(move |_| {
        let url = url_e_test.text().to_string();
        test_btn_c.set_sensitive(false);
        test_lbl.set_text("Probando conexión...");
        test_lbl.remove_css_class("badge-success");
        test_lbl.remove_css_class("badge-error");

        let test_lbl_inner = test_lbl.clone();
        let btn_inner = test_btn_c.clone();

        glib::spawn_future_local(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let start = Instant::now();
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let res = rt.block_on(async {
                    let (conn, engine, _) = ConnectionManager::create_connection(&url).await?;
                    conn.ping().await?;
                    Ok::<_, lyra_core::CoreError>((engine, start.elapsed().as_millis()))
                });
                let _ = tx.send(res);
            });

            if let Ok(res) = rx.await {
                match res {
                    Ok((engine, elapsed)) => {
                        test_lbl_inner
                            .set_text(&format!("✔ Conexión {engine} exitosa ({elapsed} ms)"));
                        test_lbl_inner.add_css_class("badge-success");
                    }
                    Err(e) => {
                        test_lbl_inner.set_text(&format!("❌ {e}"));
                        test_lbl_inner.add_css_class("badge-error");
                    }
                }
            }
            btn_inner.set_sensitive(true);
        });
    });

    // Guardar y conectar
    let on_connected_rc = Rc::new(on_connected);
    let dlg_for_conn = dialog.clone();
    let name_for_conn = name_entry;
    let url_for_conn = url_entry;

    connect_btn.connect_clicked(move |btn| {
        let name = name_for_conn.text().to_string();
        let url = url_for_conn.text().to_string();
        btn.set_sensitive(false);
        btn.set_label("Conectando...");

        let mgr_cell = mgr_cell.clone();
        let on_conn = on_connected_rc.clone();
        let dlg = dlg_for_conn.clone();
        let btn_c = btn.clone();

        glib::spawn_future_local(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let url_c = url.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let res = rt.block_on(ConnectionManager::create_connection(&url_c));
                let _ = tx.send(res);
            });

            if let Ok(res) = rx.await {
                match res {
                    Ok((conn, engine, icon_name)) => {
                        let conn_id = format!(
                            "conn_{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                        );
                        let info = ConnectionInfo {
                            id: conn_id,
                            name,
                            engine,
                            icon_name,
                            connection: conn,
                        };

                        if let Some(ref mut m) = *mgr_cell.borrow_mut() {
                            m.add_info(info.clone());
                        }

                        on_conn(info);
                        dlg.close();
                    }
                    Err(e) => {
                        btn_c.set_label("Error");
                        btn_c.set_sensitive(true);
                        eprintln!("Error al conectar: {e}");
                    }
                }
            }
        });
    });

    dialog.present();
}
