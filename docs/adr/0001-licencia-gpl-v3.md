# 1. Elección de licencia GPL-3.0-or-later

Fecha: 2026-11-01

## Estado

Aceptado

## Contexto

Lyra es una aplicación de escritorio nativa para Linux desarrollada en Rust con GTK4 y libadwaita. El proyecto se dirige a desarrolladores de software y equipos técnicos que valoran la privacidad, el rendimiento y el software libre. Era necesario definir la licencia de distribución del proyecto, considerando principalmente dos alternativas: MPL-2.0 (copyleft débil a nivel de archivo) y GPL-3.0 (copyleft fuerte).

## Decisión

Se adopta **GNU General Public License v3.0 or later (GPL-3.0-or-later)** como la licencia oficial del proyecto.

Motivos principales:
1. **Naturaleza de aplicación de escritorio:** Lyra es un producto de software final para el usuario, no una librería de propósito general. La GPL-3.0 asegura que cualquier redistribución o modificación conserve el código abierto.
2. **Alineación con el ecosistema Linux y GNOME:** El ecosistema de aplicaciones nativas de Linux y GNOME tiene una tradición y confianza arraigada en la GPL.
3. **Protección contra patentes y tivoización:** La versión 3 de la GPL incluye cláusulas explícitas sobre represalias por patentes y garantiza que el software pueda ejecutarse y modificarse sin restricciones arbitrarias de hardware.
4. **Compatibilidad con GTK4:** GTK4 utiliza LGPL-2.1+, la cual es completamente compatible con GPL-3.0.

## Consecuencias

- Todas las contribuciones al repositorio principal se integran bajo los términos de GPL-3.0-or-later.
- Se configura `cargo-deny` (`deny.toml`) para permitir únicamente dependencias con licencias compatibles con GPL-3.0 (MIT, Apache-2.0, BSD-3-Clause, LGPL, etc.).
