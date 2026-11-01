use libadwaita::glib;
use libadwaita::gtk::prelude::*;
use libadwaita::gtk::{
    self, Align, Box as GtkBox, Button, Entry, Grid, Label, Orientation, PolicyType,
    ScrolledWindow, TextView,
};
use libadwaita::{ViewStack, ViewSwitcher};
use lyra_core::{DatabaseConnection, QueryResult, TableSchema};
use std::rc::Rc;
use std::sync::Arc;

pub struct TableWorkspace {
    pub container: GtkBox,
}

impl TableWorkspace {
    pub fn new(table_name: &str, engine: &str, conn: Arc<dyn DatabaseConnection>) -> Self {
        let root = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .vexpand(true)
            .hexpand(true)
            .build();

        // 1. Barra superior: Título, Switcher de Vistas y Acciones
        let top_bar = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .margin_start(16)
            .margin_end(16)
            .margin_top(10)
            .margin_bottom(10)
            .build();

        // Título e Ícono
        let title_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();

        let icon_name = if engine == "MongoDB" {
            "emblem-documents-symbolic"
        } else if engine == "Redis" {
            "view-list-bullet-symbolic"
        } else {
            "table-symbolic"
        };
        let icon_img = gtk::Image::from_icon_name(icon_name);

        let table_title_lbl = Label::builder()
            .label(table_name)
            .css_classes(["bold-title"])
            .build();

        let engine_badge = Label::builder()
            .label(engine)
            .css_classes(["badge-engine"])
            .build();

        title_box.append(&icon_img);
        title_box.append(&table_title_lbl);
        title_box.append(&engine_badge);

        // ViewStack para alternar entre Datos, SQL y Estructura
        let view_stack = ViewStack::new();
        view_stack.set_vexpand(true);
        view_stack.set_hexpand(true);

        let view_switcher = ViewSwitcher::builder()
            .stack(&view_stack)
            .policy(libadwaita::ViewSwitcherPolicy::Wide)
            .build();

        // Acciones derecha: Refrescar y Filtro
        let actions_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .halign(Align::End)
            .hexpand(true)
            .build();

        let filter_entry = Entry::builder()
            .placeholder_text("🔍 Filtrar registros...")
            .width_chars(20)
            .build();

        let refresh_btn = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Recargar datos")
            .build();

        actions_box.append(&filter_entry);
        actions_box.append(&refresh_btn);

        top_bar.append(&title_box);
        top_bar.append(&view_switcher);
        top_bar.append(&actions_box);

        // ==========================================
        // VISTA 1: DATOS (Data Grid interactiva)
        // ==========================================
        let (data_page_box, data_status_lbl, reload_data_trigger) =
            build_data_view(table_name, engine, conn.clone());

        let data_page = view_stack.add_titled(&data_page_box, Some("data"), "Datos");
        data_page.set_icon_name(Some("view-list-symbolic"));

        // ==========================================
        // VISTA 2: CONSULTA (Consola SQL / Query)
        // ==========================================
        let sql_page_box = build_sql_view(table_name, engine, conn.clone());
        let sql_page = view_stack.add_titled(&sql_page_box, Some("sql"), "Consulta");
        sql_page.set_icon_name(Some("utilities-terminal-symbolic"));

        // ==========================================
        // VISTA 3: ESTRUCTURA (Esquema y Relaciones)
        // ==========================================
        let struct_page_box = build_structure_view(table_name, conn.clone());
        let struct_page = view_stack.add_titled(&struct_page_box, Some("structure"), "Estructura");
        struct_page.set_icon_name(Some("format-indent-more-symbolic"));

        // Conectar botón refrescar
        let reload_trigger = reload_data_trigger.clone();
        refresh_btn.connect_clicked(move |_| {
            reload_trigger();
        });

        // Conectar filtro al presionar Enter
        let reload_trigger_filter = reload_data_trigger.clone();
        filter_entry.connect_activate(move |_| {
            reload_trigger_filter();
        });

        // 3. Barra de estado inferior
        let status_bar = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .css_classes(["status-bar-box"])
            .build();

        let env_lbl = Label::builder()
            .label("🟢 Conectado")
            .css_classes(["badge-dev"])
            .build();

        status_bar.append(&env_lbl);
        status_bar.append(&data_status_lbl);

        root.append(&top_bar);
        root.append(&view_stack);
        root.append(&status_bar);

        // Cargar datos automáticamente al abrir
        reload_data_trigger();

        Self { container: root }
    }
}

/// Construye la vista de datos (Grilla interactiva)
fn build_data_view(
    table_name: &str,
    engine: &str,
    conn: Arc<dyn DatabaseConnection>,
) -> (GtkBox, Label, Rc<dyn Fn()>) {
    use std::rc::Rc;

    let box_container = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .vexpand(true)
        .hexpand(true)
        .build();

    let grid_container = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .vexpand(true)
        .hexpand(true)
        .margin_start(16)
        .margin_end(16)
        .margin_top(4)
        .margin_bottom(12)
        .css_classes(["grid-container"])
        .build();

    let scrolled = ScrolledWindow::builder()
        .child(&grid_container)
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .build();

    box_container.append(&scrolled);

    let status_label = Label::builder()
        .label("Cargando datos...")
        .css_classes(["subtle-label"])
        .halign(Align::Start)
        .hexpand(true)
        .build();

    let t_name = table_name.to_string();
    let eng = engine.to_string();
    let status_lbl_c = status_label.clone();
    let grid_c = grid_container.clone();

    let reload_fn = Rc::new(move || {
        let conn = conn.clone();
        let t_name = t_name.clone();
        let eng = eng.clone();
        let status_lbl = status_lbl_c.clone();
        let grid_box = grid_c.clone();

        status_lbl.set_text("Recuperando registros...");

        glib::spawn_future_local(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let eng_for_thread = eng.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let query = if eng_for_thread == "MongoDB" {
                    format!("{t_name}.find({{}})")
                } else if eng_for_thread == "Redis" {
                    format!("GET {t_name}")
                } else {
                    format!("SELECT * FROM {t_name} LIMIT 100;")
                };

                let res = rt.block_on(conn.execute(&query));
                let _ = tx.send(res);
            });

            if let Ok(res) = rx.await {
                // Limpiar resultados anteriores
                while let Some(child) = grid_box.first_child() {
                    grid_box.remove(&child);
                }

                match res {
                    Ok(query_res) => {
                        let row_count = query_res.rows.len();
                        status_lbl.set_text(&format!(
                            "Mostrando {row_count} registros • {} ms • Motor: {eng}",
                            query_res.execution_time_ms
                        ));

                        let grid_widget = render_table_grid(&query_res);
                        grid_box.append(&grid_widget);
                    }
                    Err(err) => {
                        status_lbl.set_text(&format!("Error cargando datos: {err}"));
                        let err_box = GtkBox::builder()
                            .orientation(Orientation::Vertical)
                            .spacing(8)
                            .halign(Align::Center)
                            .valign(Align::Center)
                            .vexpand(true)
                            .margin_top(40)
                            .build();

                        let err_lbl = Label::builder()
                            .label(format!("❌ {err}"))
                            .css_classes(["badge-error"])
                            .build();

                        err_box.append(&err_lbl);
                        grid_box.append(&err_box);
                    }
                }
            }
        });
    });

    (box_container, status_label, reload_fn)
}

/// Construye la vista de consola SQL
fn build_sql_view(table_name: &str, engine: &str, conn: Arc<dyn DatabaseConnection>) -> GtkBox {
    let vbox = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_start(16)
        .margin_end(16)
        .margin_top(8)
        .margin_bottom(12)
        .vexpand(true)
        .hexpand(true)
        .build();

    let default_query = if engine == "MongoDB" {
        format!("{table_name}.find({{}})")
    } else if engine == "Redis" {
        format!("GET {table_name}")
    } else {
        format!("SELECT * FROM {table_name} LIMIT 50;")
    };

    let editor_view = TextView::builder()
        .wrap_mode(gtk::WrapMode::Word)
        .monospace(true)
        .css_classes(["query-editor-view"])
        .left_margin(12)
        .right_margin(12)
        .top_margin(12)
        .bottom_margin(12)
        .height_request(110)
        .build();

    editor_view.buffer().set_text(&default_query);

    let editor_scroll = ScrolledWindow::builder()
        .child(&editor_view)
        .css_classes(["query-editor-box"])
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .min_content_height(100)
        .max_content_height(160)
        .build();

    // Barra de herramientas de ejecución
    let toolbar = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();

    let run_btn = Button::builder()
        .label("▶ Ejecutar (Ctrl+Enter)")
        .css_classes(["suggested-action", "pill"])
        .build();

    let clear_btn = Button::builder().label("Limpiar").build();

    let sql_status_lbl = Label::builder()
        .label("Listo para ejecutar")
        .css_classes(["subtle-label"])
        .halign(Align::Start)
        .hexpand(true)
        .build();

    toolbar.append(&run_btn);
    toolbar.append(&clear_btn);
    toolbar.append(&sql_status_lbl);

    // Contenedor de resultados de consulta
    let results_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .css_classes(["grid-container"])
        .vexpand(true)
        .hexpand(true)
        .build();

    let results_scroll = ScrolledWindow::builder()
        .child(&results_box)
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .build();

    vbox.append(&editor_scroll);
    vbox.append(&toolbar);
    vbox.append(&results_scroll);

    // Conectar botón limpiar
    let ed_view_c = editor_view.clone();
    clear_btn.connect_clicked(move |_| {
        ed_view_c.buffer().set_text("");
    });

    // Conectar botón ejecutar
    let ed_view_for_run = editor_view.clone();
    let res_box_c = results_box.clone();
    let status_c = sql_status_lbl.clone();

    run_btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        btn.set_label("Ejecutando...");

        let buf = ed_view_for_run.buffer();
        let start = buf.start_iter();
        let end = buf.end_iter();
        let sql = buf.text(&start, &end, false).to_string();

        let conn = conn.clone();
        let btn_clone = btn.clone();
        let res_box = res_box_c.clone();
        let status = status_c.clone();

        glib::spawn_future_local(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let res = rt.block_on(conn.execute(&sql));
                let _ = tx.send(res);
            });

            if let Ok(res) = rx.await {
                while let Some(child) = res_box.first_child() {
                    res_box.remove(&child);
                }

                match res {
                    Ok(query_res) => {
                        if query_res.columns.is_empty() {
                            status.set_text(&format!(
                                "✔ Filas afectadas: {} • {} ms",
                                query_res.rows_affected, query_res.execution_time_ms
                            ));
                        } else {
                            status.set_text(&format!(
                                "✔ {} filas devueltas • {} ms",
                                query_res.rows.len(),
                                query_res.execution_time_ms
                            ));
                            let grid_widget = render_table_grid(&query_res);
                            res_box.append(&grid_widget);
                        }
                    }
                    Err(err) => {
                        status.set_text(&format!("Error: {err}"));
                        let err_lbl = Label::builder()
                            .label(format!("❌ {err}"))
                            .css_classes(["badge-error"])
                            .margin_top(20)
                            .halign(Align::Center)
                            .build();
                        res_box.append(&err_lbl);
                    }
                }
            }

            btn_clone.set_sensitive(true);
            btn_clone.set_label("▶ Ejecutar (Ctrl+Enter)");
        });
    });

    vbox
}

/// Construye la vista de estructura del esquema (Columnas, Tipos, PKs, FKs)
fn build_structure_view(table_name: &str, conn: Arc<dyn DatabaseConnection>) -> GtkBox {
    let vbox = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_start(16)
        .margin_end(16)
        .margin_top(12)
        .margin_bottom(16)
        .vexpand(true)
        .hexpand(true)
        .build();

    let content_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .build();

    let scrolled = ScrolledWindow::builder()
        .child(&content_box)
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .build();

    vbox.append(&scrolled);

    let t_name = table_name.to_string();
    let content_c = content_box.clone();

    glib::spawn_future_local(async move {
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let res = rt.block_on(conn.describe_table(&t_name));
            let _ = tx.send(res);
        });

        if let Ok(res) = rx.await {
            match res {
                Ok(schema) => {
                    let schema_widget = render_schema_view(&schema);
                    content_c.append(&schema_widget);
                }
                Err(err) => {
                    let err_lbl = Label::builder()
                        .label(format!("No se pudo inspeccionar el esquema: {err}"))
                        .css_classes(["badge-error"])
                        .halign(Align::Center)
                        .margin_top(30)
                        .build();
                    content_c.append(&err_lbl);
                }
            }
        }
    });

    vbox
}

/// Renderiza la grilla de datos con encabezados y filas numeradas
fn render_table_grid(res: &QueryResult) -> Grid {
    let grid = Grid::builder()
        .row_spacing(0)
        .column_spacing(0)
        .hexpand(true)
        .vexpand(true)
        .build();

    // Encabezado de columna # (índice)
    let idx_header = Label::builder()
        .label("#")
        .css_classes(["grid-header-cell"])
        .halign(Align::Center)
        .build();
    grid.attach(&idx_header, 0, 0, 1, 1);

    // Encabezados de columnas de la tabla
    for (col_idx, col) in res.columns.iter().enumerate() {
        let header_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .css_classes(["grid-header-cell"])
            .build();

        let col_name_lbl = Label::builder()
            .label(&col.name)
            .halign(Align::Start)
            .build();

        let type_badge = Label::builder()
            .label(&col.type_name)
            .css_classes(["badge-type"])
            .halign(Align::End)
            .hexpand(true)
            .build();

        header_box.append(&col_name_lbl);
        header_box.append(&type_badge);

        grid.attach(&header_box, (col_idx + 1) as i32, 0, 1, 1);
    }

    // Filas de datos
    for (row_idx, row) in res.rows.iter().enumerate() {
        let is_alt = row_idx % 2 == 1;
        let cell_class = if is_alt {
            "grid-data-cell-alt"
        } else {
            "grid-data-cell"
        };

        // Número de fila
        let idx_lbl = Label::builder()
            .label((row_idx + 1).to_string())
            .css_classes(["grid-row-idx"])
            .halign(Align::End)
            .build();
        grid.attach(&idx_lbl, 0, (row_idx + 1) as i32, 1, 1);

        for (col_idx, val) in row.values.iter().enumerate() {
            let cell_lbl = Label::builder()
                .label(val.to_string())
                .css_classes([cell_class])
                .halign(Align::Start)
                .selectable(true)
                .hexpand(true)
                .build();

            grid.attach(&cell_lbl, (col_idx + 1) as i32, (row_idx + 1) as i32, 1, 1);
        }
    }

    grid
}

/// Renderiza la tabla de esquema (Columnas, Tipos, Nullable, PK, FK)
fn render_schema_view(schema: &TableSchema) -> GtkBox {
    let main_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .build();

    // Tabla de Columnas
    let grid = Grid::builder()
        .row_spacing(0)
        .column_spacing(0)
        .css_classes(["grid-container"])
        .build();

    let headers = [
        "Columna",
        "Tipo de Dato",
        "¿Permite Nulos?",
        "Clave Primaria",
        "Por Defecto",
    ];
    for (idx, title) in headers.iter().enumerate() {
        let h_lbl = Label::builder()
            .label(*title)
            .css_classes(["grid-header-cell"])
            .halign(Align::Start)
            .hexpand(true)
            .build();
        grid.attach(&h_lbl, idx as i32, 0, 1, 1);
    }

    for (row_idx, col) in schema.columns.iter().enumerate() {
        let is_alt = row_idx % 2 == 1;
        let cell_class = if is_alt {
            "grid-data-cell-alt"
        } else {
            "grid-data-cell"
        };

        // 1. Nombre
        let name_lbl = Label::builder()
            .label(&col.name)
            .css_classes([cell_class])
            .halign(Align::Start)
            .build();

        // 2. Tipo
        let type_lbl = Label::builder()
            .label(&col.data_type)
            .css_classes([cell_class, "badge-type"])
            .halign(Align::Start)
            .build();

        // 3. Nulable
        let null_text = if col.is_nullable {
            "Sí"
        } else {
            "No (Requerido)"
        };
        let null_lbl = Label::builder()
            .label(null_text)
            .css_classes([cell_class])
            .halign(Align::Start)
            .build();

        // 4. Clave Primaria
        let pk_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes([cell_class])
            .build();
        if col.is_primary_key {
            let pk_badge = Label::builder()
                .label("🔑 PK")
                .css_classes(["badge-pk"])
                .build();
            pk_box.append(&pk_badge);
        } else {
            pk_box.append(&Label::builder().label("-").build());
        }

        // 5. Default
        let def_text = col.default_value.as_deref().unwrap_or("-");
        let def_lbl = Label::builder()
            .label(def_text)
            .css_classes([cell_class])
            .halign(Align::Start)
            .build();

        let r = (row_idx + 1) as i32;
        grid.attach(&name_lbl, 0, r, 1, 1);
        grid.attach(&type_lbl, 1, r, 1, 1);
        grid.attach(&null_lbl, 2, r, 1, 1);
        grid.attach(&pk_box, 3, r, 1, 1);
        grid.attach(&def_lbl, 4, r, 1, 1);
    }

    main_box.append(&grid);

    // Sección de Relaciones y Claves Foráneas si existen
    if !schema.foreign_keys.is_empty() {
        let fk_title = Label::builder()
            .label("🔗 Relaciones y Claves Foráneas")
            .css_classes(["bold-title"])
            .halign(Align::Start)
            .margin_top(12)
            .build();
        main_box.append(&fk_title);

        let fk_list = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .build();

        for fk in &schema.foreign_keys {
            let row_box = GtkBox::builder()
                .orientation(Orientation::Horizontal)
                .spacing(12)
                .css_classes(["grid-container"])
                .margin_top(4)
                .margin_bottom(4)
                .margin_start(4)
                .margin_end(4)
                .build();

            let icon = gtk::Image::from_icon_name("emblem-symbolic-link");
            let desc = format!(
                "Columna local '{}' vinculada con tabla '{}' columna '{}'",
                fk.from_column, fk.to_table, fk.to_column
            );
            let desc_lbl = Label::builder().label(&desc).build();
            let badge = Label::builder()
                .label("🔗 Clave Foránea")
                .css_classes(["badge-fk"])
                .build();

            row_box.append(&icon);
            row_box.append(&desc_lbl);
            row_box.append(&badge);

            fk_list.append(&row_box);
        }

        main_box.append(&fk_list);
    }

    main_box
}
