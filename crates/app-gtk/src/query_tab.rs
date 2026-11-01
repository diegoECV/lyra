use libadwaita::glib;
use libadwaita::gtk::{
    self, Align, Box as GtkBox, Button, Grid, Label, Orientation, PolicyType, ScrolledWindow,
    TextView,
};
use libadwaita::prelude::*;
use lyra_core::{DatabaseConnection, QueryResult};
use std::sync::Arc;

pub struct QueryTab {
    pub container: GtkBox,
}

impl QueryTab {
    pub fn new(
        _title: &str,
        default_query: &str,
        conn: Arc<dyn DatabaseConnection>,
    ) -> (Self, Button) {
        let root = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(16)
            .margin_end(16)
            .vexpand(true)
            .hexpand(true)
            .build();

        // 1. Barra de herramientas superior
        let toolbar = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .build();

        let env_label = Label::builder()
            .label("🟢 Desarrollo")
            .css_classes(["env-badge-dev"])
            .build();

        let status_label = Label::builder()
            .label("Listo para ejecutar")
            .css_classes(["dim-label"])
            .hexpand(true)
            .halign(Align::Start)
            .build();

        let run_button = Button::builder()
            .label("▶ Ejecutar")
            .css_classes(["suggested-action", "pill"])
            .build();

        toolbar.append(&env_label);
        toolbar.append(&status_label);
        toolbar.append(&run_button);

        // 2. Editor de consulta
        let text_view = TextView::builder()
            .wrap_mode(gtk::WrapMode::Word)
            .css_classes(["query-editor"])
            .monospace(true)
            .left_margin(10)
            .right_margin(10)
            .top_margin(10)
            .bottom_margin(10)
            .height_request(100)
            .build();

        text_view.buffer().set_text(default_query);

        let editor_scroll = ScrolledWindow::builder()
            .child(&text_view)
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .max_content_height(140)
            .min_content_height(80)
            .build();

        // 3. Contenedor de resultados
        let results_container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .vexpand(true)
            .hexpand(true)
            .build();

        let results_scroll = ScrolledWindow::builder()
            .child(&results_container)
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .build();

        root.append(&toolbar);
        root.append(&editor_scroll);
        root.append(&results_scroll);

        // Evento de ejecución
        let text_view_clone = text_view.clone();
        let status_clone = status_label.clone();
        let results_box_clone = results_container.clone();
        let conn_clone = conn.clone();

        let execute_fn = move |btn: &Button| {
            btn.set_sensitive(false);
            btn.set_label("Ejecutando...");

            let buffer = text_view_clone.buffer();
            let start = buffer.start_iter();
            let end = buffer.end_iter();
            let sql = buffer.text(&start, &end, false).to_string();

            let status_lbl = status_clone.clone();
            let results_box = results_box_clone.clone();
            let btn_clone = btn.clone();
            let conn = conn_clone.clone();

            glib::spawn_future_local(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();

                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();

                    let res = rt.block_on(async { conn.execute(&sql).await });
                    let _ = tx.send(res);
                });

                if let Ok(res) = rx.await {
                    // Limpiar resultados anteriores
                    while let Some(child) = results_box.first_child() {
                        results_box.remove(&child);
                    }

                    match res {
                        Ok(query_res) => {
                            if query_res.columns.is_empty() {
                                status_lbl.set_markup(&format!(
                                    "<span foreground='#2ec27e'>✔ Éxito</span>: {} filas afectadas en {} ms",
                                    query_res.rows_affected, query_res.execution_time_ms
                                ));
                            } else {
                                status_lbl.set_markup(&format!(
                                    "<span foreground='#2ec27e'>✔ {} filas</span> devueltas en {} ms",
                                    query_res.rows.len(),
                                    query_res.execution_time_ms
                                ));

                                let table_widget = render_result_table(&query_res);
                                results_box.append(&table_widget);
                            }
                        }
                        Err(err) => {
                            status_lbl
                                .set_markup("<span foreground='#e01b24'>Error en consulta</span>");
                            let err_label = Label::builder()
                                .label(format!("❌ {err}"))
                                .css_classes(["error"])
                                .margin_top(20)
                                .halign(Align::Center)
                                .build();
                            results_box.append(&err_label);
                        }
                    }
                }

                btn_clone.set_sensitive(true);
                btn_clone.set_label("▶ Ejecutar");
            });
        };

        run_button.connect_clicked(execute_fn);

        (Self { container: root }, run_button)
    }
}

/// Genera una vista de tabla con encabezados y filas
fn render_result_table(res: &QueryResult) -> Grid {
    let grid = Grid::builder()
        .row_spacing(2)
        .column_spacing(4)
        .hexpand(true)
        .vexpand(true)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    // 1. Encabezados de columna
    for (col_idx, col) in res.columns.iter().enumerate() {
        let header_lbl = Label::builder()
            .label(&col.name)
            .css_classes(["table-header-cell"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        grid.attach(&header_lbl, col_idx as i32, 0, 1, 1);
    }

    // 2. Filas de datos
    for (row_idx, row) in res.rows.iter().enumerate() {
        let is_alt = row_idx % 2 == 1;
        let cell_class = if is_alt {
            "table-data-cell-alt"
        } else {
            "table-data-cell"
        };

        for (col_idx, val) in row.values.iter().enumerate() {
            let cell_lbl = Label::builder()
                .label(val.to_string())
                .css_classes([cell_class])
                .halign(Align::Start)
                .selectable(true)
                .build();

            grid.attach(&cell_lbl, col_idx as i32, (row_idx + 1) as i32, 1, 1);
        }
    }

    grid
}
