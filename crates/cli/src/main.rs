use anyhow::Result;
use clap::{Parser, Subcommand};
use lyra_core::{DatabaseConnection, QueryResult, TableSchema};
use lyra_driver_postgres::PostgresConnection;
use lyra_driver_sqlite::SqliteConnection;

#[derive(Parser)]
#[command(name = "lyra")]
#[command(author = "Diego")]
#[command(version = "0.1.0")]
#[command(about = "Lyra: Gestor de bases de datos nativo para Linux (SQL & NoSQL)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ejecuta una demostración en memoria con SQLite (crea tabla, inserta y consulta)
    Demo,

    /// Ejecuta una consulta SQL en SQLite o PostgreSQL
    Query {
        /// Cadena de conexión (ej. sqlite://base.db o postgres://user:pass@localhost:5432/db)
        #[arg(short, long, default_value = "sqlite::memory:")]
        db: String,

        /// Consulta SQL a ejecutar
        sql: String,
    },

    /// Lista las tablas existentes en la base de datos
    Tables {
        /// Cadena de conexión a la base de datos
        #[arg(short, long)]
        db: String,
    },

    /// Muestra la estructura y columnas de una tabla
    Describe {
        /// Cadena de conexión a la base de datos
        #[arg(short, long)]
        db: String,

        /// Nombre de la tabla a inspeccionar
        table: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Demo => run_demo().await?,
        Commands::Query { db, sql } => {
            let conn = connect_database(&db).await?;
            println!("Conectado a [{}]", conn.engine_name());

            let result = conn.execute(&sql).await?;
            print_query_result(&result);
        }
        Commands::Tables { db } => {
            let conn = connect_database(&db).await?;
            let tables = conn.list_tables().await?;

            println!(
                "Tablas encontradas en {} ({}):",
                conn.engine_name(),
                tables.len()
            );
            for t in tables {
                println!("  • {t}");
            }
        }
        Commands::Describe { db, table } => {
            let conn = connect_database(&db).await?;
            let schema = conn.describe_table(&table).await?;
            print_table_schema(&schema);
        }
    }

    Ok(())
}

/// Fábrica polimórfica que devuelve una conexión abstracta Box<dyn DatabaseConnection>
/// según el esquema de la URL proporcionada.
async fn connect_database(url: &str) -> Result<Box<dyn DatabaseConnection>> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        let conn = PostgresConnection::connect(url).await?;
        Ok(Box::new(conn))
    } else if url.starts_with("mysql://") || url.starts_with("mariadb://") {
        let conn = lyra_driver_mysql::MySqlConnection::connect(url).await?;
        Ok(Box::new(conn))
    } else if url.starts_with("mongodb://") || url.starts_with("mongodb+srv://") {
        let conn = lyra_driver_mongodb::MongoConnection::connect(url).await?;
        Ok(Box::new(conn))
    } else if url.starts_with("redis://") || url.starts_with("rediss://") {
        let conn = lyra_driver_redis::RedisConnection::connect(url).await?;
        Ok(Box::new(conn))
    } else {
        let norm = normalize_sqlite_url(url);
        let conn = SqliteConnection::connect(&norm).await?;
        Ok(Box::new(conn))
    }
}

fn normalize_sqlite_url(path: &str) -> String {
    if path.starts_with("sqlite:") {
        path.to_string()
    } else {
        format!("sqlite://{path}")
    }
}

async fn run_demo() -> Result<()> {
    println!("\x1b[1;35m╭────────────────────────────────────────────────────────╮\x1b[0m");
    println!("\x1b[1;35m│       🌟 Lyra Core & SQLite Driver - Demostración      │\x1b[0m");
    println!("\x1b[1;35m╰────────────────────────────────────────────────────────╯\x1b[0m\n");

    println!("1. Conectando a SQLite en memoria...");
    let conn = SqliteConnection::in_memory().await?;
    println!("   ✔ Motor: \x1b[1;32m{}\x1b[0m", conn.engine_name());
    conn.ping().await?;
    println!("   ✔ Ping: OK");

    println!("\n2. Creando tabla 'pedidos'...");
    let ddl = "
        CREATE TABLE pedidos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            cliente TEXT NOT NULL,
            total REAL NOT NULL,
            estado TEXT NOT NULL DEFAULT 'pendiente',
            fecha TEXT NOT NULL
        );
    ";
    conn.execute(ddl).await?;
    println!("   ✔ Tabla 'pedidos' creada.");

    println!("\n3. Insertando registros de prueba...");
    let insert = "
        INSERT INTO pedidos (cliente, total, estado, fecha) VALUES
            ('Diego Centeno', 189.90, 'completado', '2026-11-01 08:30:00'),
            ('Valeria Morales', 450.00, 'en_proceso', '2026-11-01 09:15:00'),
            ('Carlos Mendoza', 85.50, 'pendiente', '2026-11-01 09:45:00'),
            ('Sofia Castro', 1250.00, 'completado', '2026-11-01 10:00:00');
    ";
    let insert_res = conn.execute(insert).await?;
    println!(
        "   ✔ Filas insertadas: \x1b[1;32m{}\x1b[0m (en {} ms)",
        insert_res.rows_affected, insert_res.execution_time_ms
    );

    println!("\n4. Inspeccionando esquema de 'pedidos'...");
    let schema = conn.describe_table("pedidos").await?;
    print_table_schema(&schema);

    println!("\n5. Ejecutando consulta SELECT...");
    let select_sql = "SELECT id, cliente, total, estado, fecha FROM pedidos WHERE total > 100.0 ORDER BY total DESC;";
    println!("   SQL: \x1b[36m{}\x1b[0m", select_sql.trim());
    let query_res = conn.execute(select_sql).await?;
    print_query_result(&query_res);

    println!("\n✔ \x1b[1;32mDemostración completada con éxito. El núcleo y el driver de SQLite funcionan al 100%.\x1b[0m");
    Ok(())
}

fn print_table_schema(schema: &TableSchema) {
    println!("\nEsquema de la tabla: \x1b[1;33m{}\x1b[0m", schema.name);
    println!("┌──────────────────────┬──────────────────────┬────────────┬─────────────┐");
    println!("│ Columna              │ Tipo                 │ Nullable   │ Clave Prim. │");
    println!("├──────────────────────┼──────────────────────┼────────────┼─────────────┤");
    for col in &schema.columns {
        let pk_str = if col.is_primary_key { "✔ PK" } else { "-" };
        let null_str = if col.is_nullable { "SÍ" } else { "NO" };
        println!(
            "│ {:<20} │ {:<20} │ {:<10} │ {:<11} │",
            col.name, col.data_type, null_str, pk_str
        );
    }
    println!("└──────────────────────┴──────────────────────┴────────────┴─────────────┘");
}

fn print_query_result(res: &QueryResult) {
    if res.columns.is_empty() {
        println!(
            "Filas afectadas: {} (en {} ms)",
            res.rows_affected, res.execution_time_ms
        );
        return;
    }

    let mut widths: Vec<usize> = res.columns.iter().map(|c| c.name.len().max(8)).collect();
    for row in &res.rows {
        for (i, val) in row.values.iter().enumerate() {
            let len = val.to_string().len();
            if len > widths[i] {
                widths[i] = len.min(40);
            }
        }
    }

    let sep: String = widths
        .iter()
        .map(|w| "─".repeat(w + 2))
        .collect::<Vec<_>>()
        .join("┬");
    println!("┌{sep}┐");

    let header_cols: Vec<String> = res
        .columns
        .iter()
        .enumerate()
        .map(|(i, col)| format!(" {:<width$} ", col.name, width = widths[i]))
        .collect();
    println!("│{}│", header_cols.join("│"));

    let mid_sep: String = widths
        .iter()
        .map(|w| "─".repeat(w + 2))
        .collect::<Vec<_>>()
        .join("┼");
    println!("├{mid_sep}┤");

    for row in &res.rows {
        let row_cols: Vec<String> = row
            .values
            .iter()
            .enumerate()
            .map(|(i, val)| format!(" {:<width$} ", val.to_string(), width = widths[i]))
            .collect();
        println!("│{}│", row_cols.join("│"));
    }

    let bottom_sep: String = widths
        .iter()
        .map(|w| "─".repeat(w + 2))
        .collect::<Vec<_>>()
        .join("┴");
    println!("└{bottom_sep}┘");
    println!(
        "\x1b[2m{} fila(s) devuelta(s) en {} ms\x1b[0m\n",
        res.rows.len(),
        res.execution_time_ms
    );
}
