use thiserror::Error;

/// Errores tipados para el núcleo de Lyra.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Error de conexión: {0}")]
    ConnectionFailed(String),

    #[error("Error al ejecutar la consulta: {0}")]
    QueryError(String),

    #[error("Error de autenticación o secretos: {0}")]
    AuthError(String),

    #[error("La tabla o entidad '{0}' no existe")]
    NotFound(String),

    #[error("Operación no soportada por el motor: {0}")]
    UnsupportedOperation(String),

    #[error("Error de serialización o parseo: {0}")]
    Serialization(String),

    #[error("Error interno del driver: {0}")]
    Internal(String),
}
