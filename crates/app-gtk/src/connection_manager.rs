use lyra_core::DatabaseConnection;
use lyra_driver_mongodb::MongoConnection;
use lyra_driver_postgres::PostgresConnection;
use lyra_driver_redis::RedisConnection;
use lyra_driver_sqlite::SqliteConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConnectionInfo {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub engine: &'static str,
    pub icon_name: &'static str,
    pub connection: Arc<dyn DatabaseConnection>,
}

pub struct ConnectionManager {
    connections: Vec<ConnectionInfo>,
}

impl ConnectionManager {
    /// Inicializa el gestor con una conexión demo de SQLite precargada con datos realistas
    pub async fn new_with_demo() -> Result<Self, lyra_core::CoreError> {
        let mut mgr = Self {
            connections: Vec::new(),
        };

        let demo_conn = SqliteConnection::in_memory().await?;

        // 1. Crear tabla usuarios
        demo_conn
            .execute(
                "CREATE TABLE usuarios (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    nombre TEXT NOT NULL,
                    email TEXT NOT NULL,
                    pais TEXT NOT NULL
                );",
            )
            .await?;

        demo_conn
            .execute(
                "INSERT INTO usuarios (nombre, email, pais) VALUES
                    ('Diego Centeno', 'diego@ejemplo.pe', 'Perú'),
                    ('Valeria Morales', 'valeria@ejemplo.com', 'México'),
                    ('Carlos Mendoza', 'carlos@ejemplo.ar', 'Argentina'),
                    ('Sofia Castro', 'sofia@ejemplo.co', 'Colombia'),
                    ('Lucas Silva', 'lucas@ejemplo.br', 'Brasil');",
            )
            .await?;

        // 2. Crear tabla pedidos
        demo_conn
            .execute(
                "CREATE TABLE pedidos (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    usuario_id INTEGER NOT NULL,
                    total REAL NOT NULL,
                    estado TEXT NOT NULL,
                    fecha TEXT NOT NULL,
                    FOREIGN KEY (usuario_id) REFERENCES usuarios(id)
                );",
            )
            .await?;

        demo_conn
            .execute(
                "INSERT INTO pedidos (usuario_id, total, estado, fecha) VALUES
                    (1, 149.90, 'completado', '2026-11-01 08:30'),
                    (2, 450.00, 'en_proceso', '2026-11-01 09:15'),
                    (1, 85.50, 'completado', '2026-11-01 09:45'),
                    (3, 1250.00, 'completado', '2026-11-01 10:00'),
                    (4, 230.20, 'pendiente', '2026-11-01 10:30');",
            )
            .await?;

        // 3. Crear tabla productos
        demo_conn
            .execute(
                "CREATE TABLE productos (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    nombre TEXT NOT NULL,
                    precio REAL NOT NULL,
                    stock INTEGER NOT NULL
                );",
            )
            .await?;

        demo_conn
            .execute(
                "INSERT INTO productos (nombre, precio, stock) VALUES
                    ('Monitor 4K Wayland', 399.99, 15),
                    ('Teclado Mecánico Rust', 129.50, 40),
                    ('Mouse Ergonómico GNOME', 79.00, 25),
                    ('Hub USB-C Linux', 45.00, 80);",
            )
            .await?;

        mgr.connections.push(ConnectionInfo {
            id: "demo_sqlite".to_string(),
            name: "Tienda Demo (SQLite)".to_string(),
            engine: "SQLite",
            icon_name: "drive-harddisk-symbolic",
            connection: Arc::new(demo_conn),
        });

        Ok(mgr)
    }

    pub fn list(&self) -> &[ConnectionInfo] {
        &self.connections
    }

    #[allow(dead_code)]
    pub fn get(&self, id: &str) -> Option<&ConnectionInfo> {
        self.connections.iter().find(|c| c.id == id)
    }

    pub async fn create_connection(
        url: &str,
    ) -> Result<(Arc<dyn DatabaseConnection>, &'static str, &'static str), lyra_core::CoreError>
    {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            let c = PostgresConnection::connect(url).await?;
            Ok((Arc::new(c), "PostgreSQL", "network-server-symbolic"))
        } else if url.starts_with("mongodb://") || url.starts_with("mongodb+srv://") {
            let c = MongoConnection::connect(url).await?;
            Ok((Arc::new(c), "MongoDB", "emblem-documents-symbolic"))
        } else if url.starts_with("redis://") || url.starts_with("rediss://") {
            let c = RedisConnection::connect(url).await?;
            Ok((Arc::new(c), "Redis", "view-list-bullet-symbolic"))
        } else {
            let c = SqliteConnection::connect(url).await?;
            Ok((Arc::new(c), "SQLite", "drive-harddisk-symbolic"))
        }
    }

    pub fn add_info(&mut self, info: ConnectionInfo) {
        self.connections.push(info);
    }
}
