mod connection_manager;
mod query_tab;

use connection_manager::ConnectionManager;
use libadwaita::glib;
use libadwaita::gtk::prelude::*;
use libadwaita::gtk::{
    self, Align, Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation, PolicyType,
    ScrolledWindow,
};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, Application, ApplicationWindow, HeaderBar, NavigationPage, NavigationSplitView,
    PreferencesGroup, TabBar, TabView, ToolbarView, Window as AdwWindow, WindowTitle,
};
use query_tab::QueryTab;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const APP_ID: &str = "io.github.diegoecv.Lyra";

fn main() {
    tracing_subscriber::fmt::init();

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    // 1. Cargar CSS personalizado
    load_css();

    // 2. Inicializar TabView y TabBar para el área de trabajo
    let tab_view = TabView::new();
    let tab_bar = TabBar::builder().view(&tab_view).autohide(false).build();

    // 3. Crear el gestor de conexiones con la base demo
    let tab_view_rc = Rc::new(tab_view);

    // Contenedor de la barra lateral (Explorador)
    let sidebar_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let connections_group = PreferencesGroup::builder()
        .title("Bases de Datos")
        .description("Navegación tipo carpetas (SQL & NoSQL)")
        .build();

    sidebar_box.append(&connections_group);

    let sidebar_scroll = ScrolledWindow::builder()
        .child(&sidebar_box)
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .build();

    let sidebar_header = HeaderBar::builder()
        .title_widget(&WindowTitle::new("Explorador", "Proyecto Activo"))
        .show_end_title_buttons(false)
        .build();

    let new_conn_btn = Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Añadir Conexión")
        .build();
    sidebar_header.pack_start(&new_conn_btn);

    let sidebar_toolbar = ToolbarView::new();
    sidebar_toolbar.add_top_bar(&sidebar_header);
    sidebar_toolbar.set_content(Some(&sidebar_scroll));

    let sidebar_page = NavigationPage::builder()
        .title("Explorador")
        .child(&sidebar_toolbar)
        .build();

    // 4. Área de contenido principal (Pestañas y Editor)
    let content_header = HeaderBar::builder()
        .title_widget(&WindowTitle::new(
            "Lyra",
            "Gestor de bases de datos para Linux",
        ))
        .show_end_title_buttons(true)
        .build();

    let quick_search_btn = Button::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text("Paleta de Comandos (Ctrl+K)")
        .build();
    content_header.pack_end(&quick_search_btn);

    let content_toolbar = ToolbarView::new();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.add_top_bar(&tab_bar);
    content_toolbar.set_content(Some(&*tab_view_rc));

    let content_page = NavigationPage::builder()
        .title("Espacio de Trabajo")
        .child(&content_toolbar)
        .build();

    // 5. Split View responsivo de libadwaita
    let split_view = NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .min_sidebar_width(280.0)
        .max_sidebar_width(380.0)
        .collapsed(false)
        .build();

    // 6. Ventana principal
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Lyra")
        .content(&split_view)
        .default_width(1080)
        .default_height(720)
        .build();

    // Estado compartido para gestionar conexiones
    let mgr_cell: Rc<RefCell<Option<ConnectionManager>>> = Rc::new(RefCell::new(None));
    let mgr_cell_clone = mgr_cell.clone();
    let group_clone = connections_group.clone();
    let tab_view_clone = tab_view_rc.clone();

    // Cargar conexiones en segundo plano
    glib::spawn_future_local(async move {
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let res = rt.block_on(ConnectionManager::new_with_demo());
            let _ = tx.send(res);
        });

        if let Ok(Ok(mgr)) = rx.await {
            populate_sidebar(&group_clone, &mgr, &tab_view_clone).await;
            *mgr_cell_clone.borrow_mut() = Some(mgr);
        }
    });

    // Diálogo para Añadir Conexión
    let window_weak = window.downgrade();
    let mgr_for_dialog = mgr_cell.clone();
    let group_for_dialog = connections_group.clone();
    let tab_view_for_dialog = tab_view_rc.clone();

    new_conn_btn.connect_clicked(move |_| {
        if let Some(win) = window_weak.upgrade() {
            show_new_connection_dialog(
                &win,
                mgr_for_dialog.clone(),
                group_for_dialog.clone(),
                tab_view_for_dialog.clone(),
            );
        }
    });

    // Atajo Ctrl+K para búsqueda rápida
    let window_weak_k = window.downgrade();
    let mgr_for_k = mgr_cell.clone();
    let tab_view_for_k = tab_view_rc.clone();
    quick_search_btn.connect_clicked(move |_| {
        if let Some(win) = window_weak_k.upgrade() {
            show_quick_search_dialog(&win, mgr_for_k.clone(), tab_view_for_k.clone());
        }
    });

    window.present();
}

/// Llena la barra lateral con las conexiones y sus respectivas tablas/colecciones
async fn populate_sidebar(
    group: &PreferencesGroup,
    mgr: &ConnectionManager,
    tab_view: &Rc<TabView>,
) {
    for conn_info in mgr.list() {
        add_connection_to_sidebar(group, conn_info, tab_view, true).await;
    }
}

/// Añade una conexión individual a la barra lateral con sus tablas
async fn add_connection_to_sidebar(
    group: &PreferencesGroup,
    conn_info: &connection_manager::ConnectionInfo,
    tab_view: &Rc<TabView>,
    auto_open_first: bool,
) {
    let conn_row = ActionRow::builder()
        .title(&conn_info.name)
        .subtitle(conn_info.engine)
        .build();

    conn_row.add_prefix(&gtk::Image::from_icon_name(conn_info.icon_name));
    group.add(&conn_row);

    // Listar tablas/colecciones
    let conn_arc = conn_info.connection.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let tables = rt.block_on(conn_arc.list_tables());
        let _ = tx.send(tables);
    });

    if let Ok(Ok(tables)) = rx.await {
        let tables_list = ListBox::builder()
            .css_classes(["boxed-list"])
            .margin_start(16)
            .margin_end(8)
            .margin_bottom(8)
            .build();

        let mut opened_first_tab = false;

        for table in tables {
            let row = ListBoxRow::builder().build();
            let hbox = GtkBox::builder()
                .orientation(Orientation::Horizontal)
                .spacing(8)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(8)
                .margin_end(8)
                .build();

            let icon = if conn_info.engine == "MongoDB" {
                "emblem-documents-symbolic"
            } else if conn_info.engine == "Redis" {
                "view-list-bullet-symbolic"
            } else {
                "table-symbolic"
            };

            let icon_img = gtk::Image::from_icon_name(icon);
            let label = Label::builder()
                .label(&table)
                .hexpand(true)
                .halign(Align::Start)
                .build();

            hbox.append(&icon_img);
            hbox.append(&label);
            row.set_child(Some(&hbox));

            let tv_clone = tab_view.clone();
            let table_name = table.clone();
            let conn_for_tab = conn_info.connection.clone();
            let engine_name = conn_info.engine;

            row.connect_activate(move |_| {
                open_table_tab(&tv_clone, &table_name, engine_name, conn_for_tab.clone());
            });

            // Abrir automáticamente la primera tabla ('pedidos') al inicio para causar impacto visual
            if auto_open_first && !opened_first_tab && table == "pedidos" {
                open_table_tab(tab_view, &table, engine_name, conn_info.connection.clone());
                opened_first_tab = true;
            }

            tables_list.append(&row);
        }

        group.add(&tables_list);
    }
}

/// Abre una pestaña nueva con el editor SQL y la tabla de resultados
fn open_table_tab(
    tab_view: &TabView,
    table_name: &str,
    engine: &str,
    conn: Arc<dyn lyra_core::DatabaseConnection>,
) {
    let query = if engine == "MongoDB" {
        format!("{table_name}.find({{}})")
    } else if engine == "Redis" {
        format!("GET {table_name}")
    } else {
        format!("SELECT * FROM {table_name};")
    };

    let tab_title = format!("{table_name} ({engine})");
    let (tab_widget, run_btn) = QueryTab::new(&tab_title, &query, conn);

    let page = tab_view.append(&tab_widget.container);
    page.set_title(&tab_title);
    page.set_icon(Some(&libadwaita::gio::ThemedIcon::new(
        if engine == "MongoDB" {
            "emblem-documents-symbolic"
        } else if engine == "Redis" {
            "view-list-bullet-symbolic"
        } else {
            "table-symbolic"
        },
    )));

    tab_view.set_selected_page(&page);

    // Auto-ejecutar consulta inicial para poblar la vista
    run_btn.emit_clicked();
}

/// Diálogo para añadir nuevas conexiones
fn show_new_connection_dialog(
    parent: &ApplicationWindow,
    mgr_cell: Rc<RefCell<Option<ConnectionManager>>>,
    group: PreferencesGroup,
    tab_view: Rc<TabView>,
) {
    let dialog = AdwWindow::builder()
        .transient_for(parent)
        .modal(true)
        .title("Nueva Conexión")
        .default_width(480)
        .default_height(340)
        .build();

    let content_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let header = HeaderBar::builder()
        .title_widget(&WindowTitle::new("Añadir Base de Datos", "SQL o NoSQL"))
        .show_end_title_buttons(true)
        .build();

    let name_entry = Entry::builder()
        .placeholder_text("Nombre de la conexión (ej. PostgreSQL Local)")
        .text("Mi Base de Datos")
        .build();

    let url_entry = Entry::builder()
        .placeholder_text("Cadena de conexión (postgres://, mongodb://, redis://, sqlite://)")
        .text("sqlite::memory:")
        .build();

    let status_lbl = Label::builder().css_classes(["dim-label"]).build();

    let connect_btn = Button::builder()
        .label("Conectar")
        .css_classes(["suggested-action", "pill"])
        .halign(Align::End)
        .build();

    content_box.append(&name_entry);
    content_box.append(&url_entry);
    content_box.append(&status_lbl);
    content_box.append(&connect_btn);

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content_box));

    dialog.set_content(Some(&toolbar));

    let dialog_clone = dialog.clone();
    connect_btn.connect_clicked(move |btn| {
        let name = name_entry.text().to_string();
        let url = url_entry.text().to_string();
        btn.set_sensitive(false);
        btn.set_label("Conectando...");

        let mgr_c = mgr_cell.clone();
        let group_c = group.clone();
        let tv_c = tab_view.clone();
        let dlg = dialog_clone.clone();
        let btn_clone = btn.clone();

        glib::spawn_future_local(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let url_for_thread = url.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let res = rt.block_on(ConnectionManager::create_connection(&url_for_thread));
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
                        let info = connection_manager::ConnectionInfo {
                            id: conn_id,
                            name,
                            engine,
                            icon_name,
                            connection: conn,
                        };
                        add_connection_to_sidebar(&group_c, &info, &tv_c, false).await;
                        if let Some(ref mut m) = *mgr_c.borrow_mut() {
                            m.add_info(info);
                        }
                        dlg.close();
                    }
                    Err(e) => {
                        btn_clone.set_label("Error");
                        btn_clone.set_sensitive(true);
                        eprintln!("Error conectando: {e}");
                    }
                }
            }
        });
    });

    dialog.present();
}

/// Diálogo para búsqueda rápida (Ctrl+K)
fn show_quick_search_dialog(
    parent: &ApplicationWindow,
    mgr_cell: Rc<RefCell<Option<ConnectionManager>>>,
    tab_view: Rc<TabView>,
) {
    let dialog = AdwWindow::builder()
        .transient_for(parent)
        .modal(true)
        .title("Paleta de Comandos")
        .default_width(420)
        .default_height(280)
        .build();

    let vbox = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let search_entry = Entry::builder()
        .placeholder_text("Escribe el nombre de una tabla o comando...")
        .build();

    let list = ListBox::builder()
        .css_classes(["boxed-list"])
        .vexpand(true)
        .build();

    // Precargar tablas disponibles
    if let Some(ref mgr) = *mgr_cell.borrow() {
        for conn in mgr.list() {
            let engine = conn.engine;
            let conn_ref = conn.connection.clone();

            for table in ["pedidos", "usuarios", "productos"] {
                let row = ListBoxRow::builder().build();
                let row_box = GtkBox::builder()
                    .spacing(8)
                    .margin_top(6)
                    .margin_bottom(6)
                    .margin_start(8)
                    .margin_end(8)
                    .build();
                row_box.append(&gtk::Image::from_icon_name("table-symbolic"));
                row_box.append(
                    &Label::builder()
                        .label(format!("{table} ({engine})"))
                        .build(),
                );
                row.set_child(Some(&row_box));

                let tv = tab_view.clone();
                let dlg = dialog.clone();
                let t_name = table.to_string();
                let c_ref = conn_ref.clone();

                row.connect_activate(move |_| {
                    open_table_tab(&tv, &t_name, engine, c_ref.clone());
                    dlg.close();
                });

                list.append(&row);
            }
        }
    }

    vbox.append(&search_entry);
    vbox.append(&list);

    let header = HeaderBar::builder()
        .title_widget(&WindowTitle::new("Ir a tabla", "Ctrl+K"))
        .show_end_title_buttons(true)
        .build();

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&vbox));

    dialog.set_content(Some(&toolbar));
    dialog.present();
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    let css_data = include_str!("../resources/css/style.css");
    provider.load_from_data(css_data);

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
