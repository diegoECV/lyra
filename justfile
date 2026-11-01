default:
    @just --list

# Compilar todo el workspace
build:
    cargo build --workspace

# Ejecutar la aplicación gráfica
run:
    cargo run -p lyra-gtk

# Ejecutar la línea de comandos
run-cli:
    cargo run -p lyra-cli

# Formatear el código con rustfmt
fmt:
    cargo fmt --all

# Analizar con clippy sin advertencias permitidas
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Ejecutar pruebas unitarias
test:
    cargo test --workspace

# Chequeo completo (fmt + lint + test)
check: fmt lint test

# Limpiar artefactos de compilación
clean:
    cargo clean
