<div align="center">
  <img src="data/icons/hicolor/scalable/apps/io.github.diegoecv.Lyra.svg" width="128" height="128" alt="Lyra Logo" />
  <h1>Lyra</h1>
  <p><strong>Gestor de bases de datos nativo para Linux (SQL & NoSQL)</strong></p>
  <p>Construido en Rust con GTK4 + libadwaita.</p>
</div>

---

## 🌟 Visión

Lyra es un gestor de bases de datos moderno y ultraligero diseñado específicamente para Linux. A diferencia de las herramientas tradicionales que operan por conexiones aisladas, Lyra sitúa al **proyecto como la unidad central de trabajo**: un único espacio donde relacionar, diagramar y monitorear bases de datos mixtas (PostgreSQL, SQLite, MySQL, MongoDB, Redis) tanto en local como en Docker, Podman o Kubernetes.

### Características Principales

- 🚀 **Rápido y Nativo:** Desarrollado en Rust con GTK4 y `libadwaita`, integrado perfectamente con Wayland, modo oscuro y las guías de diseño de GNOME.
- 📂 **El Proyecto como Unidad:** Guarda la arquitectura de tus bases en archivos TOML limpios, versionables con Git y sin almacenar secretos en texto plano.
- 🔗 **SQL + NoSQL Unificados:** Define relaciones lógicas entre tablas SQL y colecciones de MongoDB con informes de salud automáticos (detección de registros huérfanos e incompatibilidad de tipos).
- 📊 **Diagramas Entidad-Relación:** Visualización clara de esquemas, exportación a SVG, PNG y PDF, con soporte para inferencia de campos en MongoDB.
- 🐳 **Integración con Contenedores:** Detección automática en Docker y Podman, métricas de consumo de recursos reales y entorno *sandbox* para simulación de fallos con Toxiproxy.
- 🔒 **Seguro por Defecto:** Modo solo lectura al conectar, secretos protegidos en el llavero del sistema operativo (`oo7` / `keyring`), avisos ante `DELETE`/`UPDATE` sin `WHERE` y **cero telemetría**.

---

## 🛠️ Requisitos Previos

- **Linux** (probado en Fedora con GNOME / Wayland)
- **Rust & Cargo** (versión 1.99+)
- **GTK4** (`gtk4-devel`) y **libadwaita** (`libadwaita-devel`)
- **GtkSourceView 5** (`gtksourceview5-devel`)
- **just** (`cargo install just` o vía gestor de paquetes de tu distro)

---

## 🚀 Inicio Rápido

Para compilar y verificar el proyecto localmente:

```bash
# Ver los atajos disponibles
just

# Comprobar formato, lints y tests
just check

# Ejecutar la aplicación gráfica
just run

# Ejecutar el CLI
just run-cli
```

---

## 🏗️ Estructura del Workspace

El proyecto utiliza un *workspace* modular de Cargo:

```text
lyra/
├── crates/
│   ├── core/              # Traits, tipos comunes y errores del núcleo (sin GTK)
│   ├── drivers/           # Adaptadores por motor (sqlite, postgres, mongodb, redis)
│   │   └── sqlite/
│   ├── cli/               # Binario de línea de comandos para terminal y CI
│   └── app-gtk/           # Aplicación gráfica (GTK4 + libadwaita + Relm4)
├── data/icons/            # Íconos vectoriales de la aplicación
├── docs/adr/              # Architecture Decision Records (ADRs)
└── justfile               # Automatización de tareas de desarrollo
```

---

## 📄 Licencia

Este proyecto está bajo la licencia **GNU General Public License v3.0 (GPL-3.0-or-later)**. Consulta el archivo [LICENSE](LICENSE) para más detalles.
