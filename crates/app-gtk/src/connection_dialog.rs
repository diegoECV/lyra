use crate::connection_manager::{ConnectionInfo, ConnectionManager};
use libadwaita::glib;
use libadwaita::gtk::prelude::*;
use libadwaita::gtk::{
    self, Align, Box as GtkBox, Button, Label, Orientation, PolicyType, ScrolledWindow,
};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, EntryRow, HeaderBar, PasswordEntryRow, PreferencesGroup, ToolbarView,
    Window as AdwWindow, WindowTitle,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EngineType {
    Sqlite,
    Postgres,
    Mongo,
    Redis,
}

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
        .default_width(560)
        .default_height(600)
        .build();

    let content_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .margin_start(24)
        .margin_end(24)
        .margin_top(18)
        .margin_bottom(24)
        .build();

    let scrolled = ScrolledWindow::builder()
        .child(&content_box)
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .build();

    // 1. Selector de motor con tarjetas visuales enriquecidas
    let engine_title = Label::builder()
        .label("Motor de Base de Datos")
        .halign(Align::Start)
        .css_classes(["bold-title"])
        .build();
    content_box.append(&engine_title);

    let cards_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .homogeneous(true)
        .build();

    fn create_engine_card(icon_name: &str, title: &str, subtitle: &str) -> Button {
        let card_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .halign(Align::Center)
            .valign(Align::Center)
            .build();

        let icon = gtk::Image::from_icon_name(icon_name);
        icon.set_pixel_size(24);

        let title_lbl = Label::builder()
            .label(title)
            .css_classes(["card-title"])
            .build();

        let sub_lbl = Label::builder()
            .label(subtitle)
            .css_classes(["card-subtitle"])
            .build();

        card_box.append(&icon);
        card_box.append(&title_lbl);
        card_box.append(&sub_lbl);

        Button::builder()
            .child(&card_box)
            .css_classes(["engine-card"])
            .build()
    }

    let sqlite_card = create_engine_card("drive-harddisk-symbolic", "SQLite", "Local / Memoria");
    sqlite_card.add_css_class("engine-card-selected");

    let pg_card = create_engine_card("network-server-symbolic", "PostgreSQL", "Puerto 5432");
    let mongo_card = create_engine_card("emblem-documents-symbolic", "MongoDB", "Puerto 27017");
    let redis_card = create_engine_card("view-list-bullet-symbolic", "Redis", "Puerto 6379");

    cards_box.append(&sqlite_card);
    cards_box.append(&pg_card);
    cards_box.append(&mongo_card);
    cards_box.append(&redis_card);
    content_box.append(&cards_box);

    // 2. Grupo de campos de configuración estructurada (AdwEntryRow / AdwPreferencesGroup)
    let config_group = PreferencesGroup::builder()
        .title("Parámetros de Conexión")
        .description("Credenciales y ubicación del servidor")
        .build();

    let name_row = EntryRow::builder()
        .title("Nombre de la conexión")
        .text("Base de Datos SQLite")
        .build();

    let host_row = EntryRow::builder().title("Host").text("localhost").build();

    let port_row = EntryRow::builder().title("Puerto").text("5432").build();

    let db_row = EntryRow::builder()
        .title("Base de Datos / Colección")
        .text("postgres")
        .build();

    let user_row = EntryRow::builder()
        .title("Usuario")
        .text("postgres")
        .build();

    let pass_row = PasswordEntryRow::builder().title("Contraseña").build();

    let uri_row = EntryRow::builder()
        .title("Cadena de Conexión (URI)")
        .text("sqlite::memory:")
        .build();

    config_group.add(&name_row);
    config_group.add(&host_row);
    config_group.add(&port_row);
    config_group.add(&db_row);
    config_group.add(&user_row);
    config_group.add(&pass_row);
    config_group.add(&uri_row);

    content_box.append(&config_group);

    // Estado del motor activo
    let current_engine = Rc::new(RefCell::new(EngineType::Sqlite));

    // Función para actualizar visibilidad y valores según motor
    let update_form_for_engine = {
        let host = host_row.clone();
        let port = port_row.clone();
        let db = db_row.clone();
        let user = user_row.clone();
        let pass = pass_row.clone();
        let uri = uri_row.clone();
        let name = name_row.clone();

        move |engine: EngineType| match engine {
            EngineType::Sqlite => {
                host.set_visible(false);
                port.set_visible(false);
                db.set_visible(false);
                user.set_visible(false);
                pass.set_visible(false);
                uri.set_visible(true);
                name.set_text("Base de Datos SQLite");
                uri.set_text("sqlite::memory:");
            }
            EngineType::Postgres => {
                host.set_visible(true);
                port.set_visible(true);
                db.set_visible(true);
                user.set_visible(true);
                pass.set_visible(true);
                uri.set_visible(true);
                name.set_text("PostgreSQL Servidor");
                host.set_text("localhost");
                port.set_text("5432");
                db.set_text("postgres");
                user.set_text("postgres");
                pass.set_text("");
                uri.set_text("postgres://postgres@localhost:5432/postgres");
            }
            EngineType::Mongo => {
                host.set_visible(true);
                port.set_visible(true);
                db.set_visible(true);
                user.set_visible(true);
                pass.set_visible(true);
                uri.set_visible(true);
                name.set_text("MongoDB Colecciones");
                host.set_text("localhost");
                port.set_text("27017");
                db.set_text("admin");
                user.set_text("");
                pass.set_text("");
                uri.set_text("mongodb://localhost:27017/admin");
            }
            EngineType::Redis => {
                host.set_visible(true);
                port.set_visible(true);
                db.set_visible(false);
                user.set_visible(false);
                pass.set_visible(true);
                uri.set_visible(true);
                name.set_text("Redis Caché");
                host.set_text("127.0.0.1");
                port.set_text("6379");
                pass.set_text("");
                uri.set_text("redis://127.0.0.1:6379");
            }
        }
    };

    // Inicializar con SQLite (ocultando host/port iniciales)
    update_form_for_engine(EngineType::Sqlite);

    // Función auxiliar para construir la URL calculada
    let get_effective_url = {
        let current_engine = current_engine.clone();
        let uri = uri_row.clone();
        let host = host_row.clone();
        let port = port_row.clone();
        let db = db_row.clone();
        let user = user_row.clone();
        let pass = pass_row.clone();

        move || -> String {
            let direct_uri = uri.text().trim().to_string();
            let engine = *current_engine.borrow();

            match engine {
                EngineType::Sqlite => {
                    if direct_uri.is_empty() {
                        "sqlite::memory:".to_string()
                    } else {
                        direct_uri
                    }
                }
                EngineType::Postgres => {
                    let h = host.text();
                    let p = port.text();
                    let d = db.text();
                    let u = user.text();
                    let pwd = pass.text();

                    if !direct_uri.is_empty() && direct_uri.starts_with("postgres") {
                        direct_uri
                    } else if pwd.is_empty() {
                        format!("postgres://{u}@{h}:{p}/{d}")
                    } else {
                        format!("postgres://{u}:{pwd}@{h}:{p}/{d}")
                    }
                }
                EngineType::Mongo => {
                    let h = host.text();
                    let p = port.text();
                    let d = db.text();
                    let u = user.text();
                    let pwd = pass.text();

                    if !direct_uri.is_empty() && direct_uri.starts_with("mongodb") {
                        direct_uri
                    } else if u.is_empty() {
                        format!("mongodb://{h}:{p}/{d}")
                    } else {
                        format!("mongodb://{u}:{pwd}@{h}:{p}/{d}")
                    }
                }
                EngineType::Redis => {
                    let h = host.text();
                    let p = port.text();
                    let pwd = pass.text();

                    if !direct_uri.is_empty() && direct_uri.starts_with("redis") {
                        direct_uri
                    } else if pwd.is_empty() {
                        format!("redis://{h}:{p}")
                    } else {
                        format!("redis://:{pwd}@{h}:{p}")
                    }
                }
            }
        }
    };

    // Eventos de selección de tarjeta
    let cards = [
        sqlite_card.clone(),
        pg_card.clone(),
        mongo_card.clone(),
        redis_card.clone(),
    ];
    let select_card = {
        let cards = cards.clone();
        move |selected_idx: usize| {
            for (idx, card) in cards.iter().enumerate() {
                if idx == selected_idx {
                    card.add_css_class("engine-card-selected");
                } else {
                    card.remove_css_class("engine-card-selected");
                }
            }
        }
    };

    {
        let sel = select_card.clone();
        let update = update_form_for_engine.clone();
        let curr = current_engine.clone();
        sqlite_card.connect_clicked(move |_| {
            sel(0);
            *curr.borrow_mut() = EngineType::Sqlite;
            update(EngineType::Sqlite);
        });
    }

    {
        let sel = select_card.clone();
        let update = update_form_for_engine.clone();
        let curr = current_engine.clone();
        pg_card.connect_clicked(move |_| {
            sel(1);
            *curr.borrow_mut() = EngineType::Postgres;
            update(EngineType::Postgres);
        });
    }

    {
        let sel = select_card.clone();
        let update = update_form_for_engine.clone();
        let curr = current_engine.clone();
        mongo_card.connect_clicked(move |_| {
            sel(2);
            *curr.borrow_mut() = EngineType::Mongo;
            update(EngineType::Mongo);
        });
    }

    {
        let sel = select_card;
        let update = update_form_for_engine;
        let curr = current_engine.clone();
        redis_card.connect_clicked(move |_| {
            sel(3);
            *curr.borrow_mut() = EngineType::Redis;
            update(EngineType::Redis);
        });
    }

    // 3. Grupo de Diagnóstico y Prueba
    let diag_group = PreferencesGroup::builder()
        .title("Diagnóstico de Conectividad")
        .description("Comprueba si el servidor está activo y responde")
        .build();

    let test_action_row = ActionRow::builder()
        .title("Probar Conexión")
        .subtitle("Pulsa 'Probar' para verificar la latencia y credenciales")
        .build();
    test_action_row.add_prefix(&gtk::Image::from_icon_name(
        "network-transmit-receive-symbolic",
    ));

    let test_btn = Button::builder()
        .label("⚡ Probar")
        .valign(Align::Center)
        .build();

    test_action_row.add_suffix(&test_btn);
    diag_group.add(&test_action_row);
    content_box.append(&diag_group);

    // Evento de prueba de conexión
    let get_url_for_test = get_effective_url.clone();
    let row_c = test_action_row.clone();
    let btn_c = test_btn.clone();

    test_btn.connect_clicked(move |_| {
        let url = get_url_for_test();
        btn_c.set_sensitive(false);
        row_c.set_subtitle("Conectando con el servidor...");

        let row_inner = row_c.clone();
        let btn_inner = btn_c.clone();

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
                        row_inner.set_subtitle(&format!(
                            "✔ Conexión exitosa a {engine} (Latencia: {elapsed} ms)"
                        ));
                    }
                    Err(e) => {
                        row_inner.set_subtitle(&format!("❌ Fallo de conexión: {e}"));
                    }
                }
            }
            btn_inner.set_sensitive(true);
        });
    });

    // 4. Encabezado con Cancelar y Conectar
    let cancel_btn = Button::builder().label("Cancelar").build();
    let connect_btn = Button::builder()
        .label("Conectar")
        .css_classes(["suggested-action", "pill"])
        .build();

    let header = HeaderBar::builder()
        .title_widget(&WindowTitle::new("Añadir Base de Datos", "SQL & NoSQL"))
        .show_end_title_buttons(false)
        .build();
    header.pack_start(&cancel_btn);
    header.pack_end(&connect_btn);

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scrolled));
    dialog.set_content(Some(&toolbar));

    // Cancelar
    let dlg_c = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dlg_c.close();
    });

    // Conectar y guardar
    let on_connected_rc = Rc::new(on_connected);
    let dlg_for_conn = dialog.clone();
    let name_for_conn = name_row;
    let get_url_for_conn = get_effective_url;

    connect_btn.connect_clicked(move |btn| {
        let name = name_for_conn.text().trim().to_string();
        let url = get_url_for_conn();
        btn.set_sensitive(false);
        btn.set_label("Conectando...");

        let mgr_cell = mgr_cell.clone();
        let on_conn = on_connected_rc.clone();
        let dlg = dlg_for_conn.clone();
        let btn_inner = btn.clone();

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
                        btn_inner.set_label("Error");
                        btn_inner.set_sensitive(true);
                        eprintln!("Error al conectar: {e}");
                    }
                }
            }
        });
    });

    dialog.present();
}
