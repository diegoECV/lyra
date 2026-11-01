use libadwaita::glib;
use libadwaita::gtk::{self, Box as GtkBox, Button, Label, Orientation};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, Application, ApplicationWindow, Clamp, HeaderBar, PreferencesGroup, StatusPage,
    ToolbarView, WindowTitle,
};
use lyra_core::DatabaseConnection;
use lyra_driver_sqlite::SqliteConnection;

const APP_ID: &str = "io.github.diegoecv.Lyra";

fn main() {
    tracing_subscriber::fmt::init();

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    // 1. Cabecera Adwaita nativa
    let header = HeaderBar::builder()
        .title_widget(&WindowTitle::new(
            "Lyra",
            "Gestor de bases de datos para Linux",
        ))
        .show_end_title_buttons(true)
        .build();

    // 2. Página de estado principal (con &amp; escapado para Pango markup)
    let status_page = StatusPage::builder()
        .icon_name("network-server-symbolic")
        .title("Lyra")
        .description("Entorno listo. Gestor nativo para desarrolladores (SQL &amp; NoSQL).")
        .build();

    // 3. Grupo de información del sistema y estado
    let group = PreferencesGroup::builder()
        .title("Estado del Entorno")
        .description("Verificación de la pila tecnológica de escritorio")
        .build();

    let row_stack = ActionRow::builder()
        .title("Pila Nativa")
        .subtitle("Rust 1.99.1 • GTK4 4.22 • libadwaita 1.9 • Wayland")
        .build();
    row_stack.add_prefix(&gtk::Image::from_icon_name(
        "software-update-available-symbolic",
    ));
    group.add(&row_stack);

    let row_drivers = ActionRow::builder()
        .title("Drivers Multi-Motor")
        .subtitle("SQLite, PostgreSQL, MongoDB y Redis listos")
        .build();
    row_drivers.add_prefix(&gtk::Image::from_icon_name("drive-harddisk-symbolic"));
    group.add(&row_drivers);

    // 4. Botón interactivo para probar el backend en vivo
    let test_button = Button::builder()
        .label("⚡ Probar conexión SQLite en memoria")
        .css_classes(["suggested-action", "pill"])
        .margin_top(18)
        .margin_bottom(12)
        .halign(gtk::Align::Center)
        .build();

    let result_label = Label::builder()
        .label("Haz clic en el botón para verificar la comunicación asíncrona.")
        .css_classes(["dim-label"])
        .halign(gtk::Align::Center)
        .build();

    let lbl_clone = result_label.clone();
    test_button.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        btn.set_label("Conectando...");
        let lbl = lbl_clone.clone();
        let btn_ref = btn.clone();

        glib::spawn_future_local(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();

            // Tarea en segundo plano para no congelar la interfaz
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let res = rt.block_on(async {
                    let conn = SqliteConnection::in_memory().await?;
                    conn.execute("CREATE TABLE demo (id INTEGER PRIMARY KEY, item TEXT);")
                        .await?;
                    conn.execute(
                        "INSERT INTO demo (item) VALUES ('Postgres'), ('Mongo'), ('Redis');",
                    )
                    .await?;
                    let count = conn.execute("SELECT * FROM demo;").await?.rows.len();
                    Ok::<usize, lyra_core::CoreError>(count)
                });

                let _ = tx.send(res.map_err(|e| e.to_string()));
            });

            if let Ok(test_result) = rx.await {
                match test_result {
                    Ok(count) => {
                        lbl.set_markup(&format!(
                            "<span foreground='#2ec27e'><b>✔ SQLite conectado con éxito</b></span>: {} registros creados y consultados.",
                            count
                        ));
                        btn_ref.set_label("⚡ Volver a probar");
                    }
                    Err(e) => {
                        lbl.set_markup(&format!(
                            "<span foreground='#e01b24'><b>Error:</b></span> {}",
                            e
                        ));
                        btn_ref.set_label("⚡ Reintentar");
                    }
                }
            }

            btn_ref.set_sensitive(true);
        });
    });

    // 5. Contenedor con Clamp para centrado responsivo
    let content_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    content_box.append(&status_page);
    content_box.append(&group);
    content_box.append(&test_button);
    content_box.append(&result_label);

    let clamp = Clamp::builder()
        .maximum_size(600)
        .child(&content_box)
        .build();

    let scroll = gtk::ScrolledWindow::builder()
        .child(&clamp)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .hexpand(true)
        .build();

    // 6. ToolbarView: el contenedor canónico de libadwaita para cabecera + contenido
    let toolbar_view = ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&scroll));

    // 7. Ventana de libadwaita
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Lyra")
        .content(&toolbar_view)
        .default_width(720)
        .default_height(640)
        .build();

    window.present();
}
