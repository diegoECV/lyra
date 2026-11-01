use libadwaita::glib;
use libadwaita::gtk::prelude::*;
use libadwaita::gtk::{
    self, Align, Box as GtkBox, Button, Entry, Grid, Label, Orientation, PolicyType,
    ScrolledWindow, TextView,
};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, EntryRow, HeaderBar, PreferencesGroup, ToolbarView, ViewStack, ViewSwitcher,
    Window as AdwWindow, WindowTitle,
};
use lyra_core::{DatabaseConnection, QueryResult, TableSchema};
use std::cell::RefCell;
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

        // Acciones derecha: Filtro, Exportar y Refrescar
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

        let export_btn = Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text("Exportar datos a CSV o JSON")
            .build();

        let refresh_btn = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Recargar datos")
            .build();

        actions_box.append(&filter_entry);
        actions_box.append(&export_btn);
        actions_box.append(&refresh_btn);

        top_bar.append(&title_box);
        top_bar.append(&view_switcher);
        top_bar.append(&actions_box);

        // ==========================================
        // VISTA 1: DATOS (Data Grid interactiva)
        // ==========================================
        let data_view = build_data_view(table_name, engine, conn.clone());

        let data_page = view_stack.add_titled(&data_view.container, Some("data"), "Datos");
        data_page.set_icon_name(Some("view-list-symbolic"));

        let reload_data_trigger = data_view.reload_fn.clone();
        let current_result_cell = data_view.current_result;
        let grid_box_ref = data_view.grid_container;
        let data_status_lbl = data_view.status_label;

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

        // Filtro en tiempo real al escribir en la barra de búsqueda
        let current_res_filter = current_result_cell.clone();
        let grid_filter = grid_box_ref;
        let status_filter = data_status_lbl.clone();
        let eng_filter = engine.to_string();

        filter_entry.connect_changed(move |entry| {
            let query = entry.text().trim().to_lowercase();
            let res_guard = current_res_filter.borrow();
            if let Some(ref res) = *res_guard {
                while let Some(child) = grid_filter.first_child() {
                    grid_filter.remove(&child);
                }

                if query.is_empty() {
                    let grid_widget = render_table_grid(res);
                    grid_filter.append(&grid_widget);
                    status_filter.set_text(&format!(
                        "Mostrando {} registros • {} ms • Motor: {eng_filter}",
                        res.rows.len(),
                        res.execution_time_ms
                    ));
                } else {
                    let filtered_rows: Vec<_> = res
                        .rows
                        .iter()
                        .filter(|row| {
                            row.values
                                .iter()
                                .any(|v| v.to_string().to_lowercase().contains(&query))
                        })
                        .cloned()
                        .collect();

                    let count = filtered_rows.len();
                    let mut filtered_res = res.clone();
                    filtered_res.rows = filtered_rows;

                    let grid_widget = render_table_grid(&filtered_res);
                    grid_filter.append(&grid_widget);
                    status_filter.set_text(&format!(
                        "Mostrando {count} de {} registros (filtro: \"{query}\") • Motor: {eng_filter}",
                        res.rows.len()
                    ));
                }
            }
        });

        // Botón exportar datos (CSV / JSON)
        let current_res_export = current_result_cell;
        let t_name_export = table_name.to_string();
        let root_for_export = root.clone();

        export_btn.connect_clicked(move |_| {
            show_export_dialog(&root_for_export, &t_name_export, current_res_export.clone());
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

struct DataViewComponents {
    container: GtkBox,
    status_label: Label,
    reload_fn: Rc<dyn Fn()>,
    grid_container: GtkBox,
    current_result: Rc<RefCell<Option<QueryResult>>>,
}

/// Construye la vista de datos (Grilla interactiva)
fn build_data_view(
    table_name: &str,
    engine: &str,
    conn: Arc<dyn DatabaseConnection>,
) -> DataViewComponents {
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
    let current_res_cell: Rc<RefCell<Option<QueryResult>>> = Rc::new(RefCell::new(None));
    let current_res_cell_clone = current_res_cell.clone();

    let reload_fn = Rc::new(move || {
        let conn = conn.clone();
        let t_name = t_name.clone();
        let eng = eng.clone();
        let status_lbl = status_lbl_c.clone();
        let grid_box = grid_c.clone();
        let res_cell = current_res_cell_clone.clone();

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

                        *res_cell.borrow_mut() = Some(query_res.clone());

                        let grid_widget = render_table_grid(&query_res);
                        grid_box.append(&grid_widget);
                    }
                    Err(err) => {
                        *res_cell.borrow_mut() = None;
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

    DataViewComponents {
        container: box_container,
        status_label,
        reload_fn,
        grid_container,
        current_result: current_res_cell,
    }
}

/// Diálogo para exportar datos en formato CSV o JSON
fn show_export_dialog(
    parent: &GtkBox,
    table_name: &str,
    current_res: Rc<RefCell<Option<QueryResult>>>,
) {
    let root_win = parent.root().and_downcast::<AdwWindow>();

    let dialog = AdwWindow::builder()
        .modal(true)
        .title("Exportar Datos")
        .default_width(480)
        .default_height(320)
        .build();

    if let Some(ref win) = root_win {
        dialog.set_transient_for(Some(win));
    }

    let vbox = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_start(20)
        .margin_end(20)
        .margin_top(16)
        .margin_bottom(20)
        .build();

    let pref_group = PreferencesGroup::builder()
        .title("Opciones de Exportación")
        .description(format!("Exportando datos de la tabla: {table_name}"))
        .build();

    // Selector de formato (CSV o JSON)
    let format_row = ActionRow::builder()
        .title("Formato")
        .subtitle("Selecciona el formato de archivo de salida")
        .build();

    let format_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .valign(Align::Center)
        .build();

    let csv_btn = Button::builder()
        .label("📄 CSV")
        .css_classes(["engine-card", "engine-card-selected"])
        .build();

    let json_btn = Button::builder()
        .label("📋 JSON")
        .css_classes(["engine-card"])
        .build();

    format_box.append(&csv_btn);
    format_box.append(&json_btn);
    format_row.add_suffix(&format_box);
    pref_group.add(&format_row);

    // Ruta de destino por defecto en ~/Descargas o HOME
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let default_dir = if std::path::Path::new(&format!("{home}/Descargas")).exists() {
        format!("{home}/Descargas")
    } else {
        home
    };

    let path_row = EntryRow::builder()
        .title("Ruta del archivo")
        .text(format!("{default_dir}/{table_name}.csv"))
        .build();
    pref_group.add(&path_row);

    let status_lbl = Label::builder().label("").halign(Align::Start).build();

    vbox.append(&pref_group);
    vbox.append(&status_lbl);

    let is_json = Rc::new(RefCell::new(false));

    // Cambiar a formato CSV
    {
        let is_json_c = is_json.clone();
        let path_r = path_row.clone();
        let csv_b = csv_btn.clone();
        let json_b = json_btn.clone();
        let def_dir = default_dir.clone();
        let t_name = table_name.to_string();

        csv_btn.connect_clicked(move |_| {
            *is_json_c.borrow_mut() = false;
            csv_b.add_css_class("engine-card-selected");
            json_b.remove_css_class("engine-card-selected");
            path_r.set_text(&format!("{def_dir}/{t_name}.csv"));
        });
    }

    // Cambiar a formato JSON
    {
        let is_json_c = is_json.clone();
        let path_r = path_row.clone();
        let csv_b = csv_btn;
        let json_b = json_btn;
        let def_dir = default_dir;
        let t_name = table_name.to_string();
        let json_b_inner = json_b.clone();
        json_b.connect_clicked(move |_| {
            *is_json_c.borrow_mut() = true;
            json_b_inner.add_css_class("engine-card-selected");
            csv_b.remove_css_class("engine-card-selected");
            path_r.set_text(&format!("{def_dir}/{t_name}.json"));
        });
    }

    // Botones HeaderBar
    let cancel_btn = Button::builder().label("Cancelar").build();
    let save_btn = Button::builder()
        .label("Guardar")
        .css_classes(["suggested-action", "pill"])
        .build();

    let header = HeaderBar::builder()
        .title_widget(&WindowTitle::new("Exportar", table_name))
        .show_end_title_buttons(false)
        .build();
    header.pack_start(&cancel_btn);
    header.pack_end(&save_btn);

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&vbox));
    dialog.set_content(Some(&toolbar));

    let dlg_c = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dlg_c.close();
    });

    // Guardar archivo en disco
    let dlg_save = dialog.clone();
    let status_save = status_lbl;
    let path_save = path_row;
    let is_json_save = is_json;

    save_btn.connect_clicked(move |_| {
        let path = path_save.text().trim().to_string();
        let res_guard = current_res.borrow();
        if let Some(ref res) = *res_guard {
            let content = if *is_json_save.borrow() {
                match res.to_json_pretty() {
                    Ok(json) => json,
                    Err(e) => {
                        status_save.set_text(&format!("❌ Error serializando JSON: {e}"));
                        status_save.add_css_class("badge-error");
                        return;
                    }
                }
            } else {
                res.to_csv()
            };

            match std::fs::write(&path, content) {
                Ok(_) => {
                    dlg_save.close();
                }
                Err(e) => {
                    status_save.set_text(&format!("❌ Error al escribir archivo: {e}"));
                    status_save.add_css_class("badge-error");
                }
            }
        } else {
            status_save.set_text("No hay datos cargados para exportar");
            status_save.add_css_class("badge-error");
        }
    });

    dialog.present();
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
    let ed_view_for_run = editor_view;
    let res_box_c = results_box;
    let status_c = sql_status_lbl;

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
    let content_c = content_box;

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
