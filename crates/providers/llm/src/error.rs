use thiserror::Error;

/// Deliberately specific instead of a single generic "request failed" —
/// sección 100 del brief: "no mostrar 'Error desconocido' si podemos
/// proporcionar algo útil."
#[derive(Debug, Clone, Error)]
pub enum ProviderError {
    #[error("no se pudo conectar — ¿está el servidor corriendo en la URL configurada?")]
    ConnectionRefused,

    #[error("la solicitud superó el tiempo de espera")]
    Timeout,

    #[error("no autorizado — revisa la API key")]
    Unauthorized,

    #[error("límite de solicitudes alcanzado, intenta de nuevo en unos segundos")]
    RateLimited,

    /// Distinto de `RateLimited`: esto es "se acabó el crédito/cupo de la
    /// cuenta", no "estás pidiendo demasiado rápido" — cada adaptador lo
    /// detecta sniffeando el cuerpo de su propio proveedor (un 429 genérico
    /// no alcanza para distinguirlos, ver `openai_compatible::check_status`).
    /// Ver `should_fail_over` para qué errores hacen que `FailoverLLMProvider`
    /// saque al proveedor "en uso" de turno.
    #[error("se agotó el crédito de esta cuenta")]
    QuotaExceeded(String),

    #[error("el modelo '{0}' no está disponible en este servidor")]
    ModelUnavailable(String),

    #[error("respuesta inválida del proveedor: {0}")]
    InvalidResponse(String),

    #[error("la solicitud falló: {0}")]
    Request(String),

    /// El modo de privacidad de este proveedor no permite lo que la petición
    /// manda (p. ej. fusionar memorias con un proveedor que no puede verlas).
    /// Nunca cambia de proveedor solo: el usuario tiene que decidirlo.
    #[error("el modo de privacidad de este proveedor no permite esta operación: {0}")]
    PrivacyBlocked(String),
}

impl ProviderError {
    /// `true` si el proveedor no puede atender *ahora* por algo ajeno a la
    /// configuración del usuario — sin crédito, límite de solicitudes, servidor
    /// apagado o sin responder, error 5xx — y por eso `FailoverLLMProvider`
    /// puede pasar al siguiente proveedor fijado sin esconder un problema real.
    ///
    /// `false` para lo que cambiar de proveedor NO arregla y el usuario tiene
    /// que ver tal cual: una key inválida o un modelo que no existe.
    pub fn should_fail_over(&self) -> bool {
        self.failover_reason().is_some()
    }

    /// Por qué este error deja al proveedor sin poder atender, o `None` si es un
    /// error que cambiar de proveedor no arregla (ver `should_fail_over`).
    pub fn failover_reason(&self) -> Option<FailoverReason> {
        match self {
            ProviderError::QuotaExceeded(_) => Some(FailoverReason::QuotaExceeded),
            ProviderError::RateLimited => Some(FailoverReason::RateLimited),
            ProviderError::ConnectionRefused | ProviderError::Request(_) => Some(FailoverReason::Unreachable),
            ProviderError::Timeout => Some(FailoverReason::Timeout),
            // Los adaptadores devuelven los demás HTTP no exitosos como
            // `InvalidResponse("HTTP 503 Service Unavailable: ...")`.
            ProviderError::InvalidResponse(message) if message.starts_with("HTTP 5") => Some(FailoverReason::ServerError),
            ProviderError::InvalidResponse(_)
            | ProviderError::Unauthorized
            | ProviderError::ModelUnavailable(_)
            | ProviderError::PrivacyBlocked(_) => None,
        }
    }
}

/// Por qué la app pasó a otro proveedor (o volvió al preferido). Se publica en
/// `AppEvent::LlmProviderSwitched` y la interfaz lo traduce a un aviso claro
/// ("Groq activado porque LM Studio no respondió").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FailoverReason {
    /// Se agotó el crédito de la cuenta.
    QuotaExceeded,
    /// Límite de solicitudes o de tokens del plan.
    RateLimited,
    /// Servidor apagado o inalcanzable.
    Unreachable,
    /// Aceptó la petición pero no respondió a tiempo.
    Timeout,
    /// Error 5xx del servidor.
    ServerError,
    /// No es un fallo: el proveedor preferido volvió a responder y se recupera.
    Restored,
}

impl FailoverReason {
    /// Valor estable que viaja en el evento hacia el frontend.
    pub fn as_str(self) -> &'static str {
        match self {
            FailoverReason::QuotaExceeded => "quota_exceeded",
            FailoverReason::RateLimited => "rate_limited",
            FailoverReason::Unreachable => "unreachable",
            FailoverReason::Timeout => "timeout",
            FailoverReason::ServerError => "server_error",
            FailoverReason::Restored => "restored",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outages_and_limits_fail_over_but_misconfiguration_does_not() {
        assert!(ProviderError::QuotaExceeded(String::new()).should_fail_over());
        assert!(ProviderError::RateLimited.should_fail_over());
        assert!(ProviderError::ConnectionRefused.should_fail_over());
        assert!(ProviderError::Timeout.should_fail_over());
        assert!(ProviderError::Request("reset".into()).should_fail_over());
        assert!(ProviderError::InvalidResponse("HTTP 503 Service Unavailable: x".into()).should_fail_over());

        assert!(!ProviderError::Unauthorized.should_fail_over());
        assert!(!ProviderError::ModelUnavailable("m".into()).should_fail_over());
        assert!(!ProviderError::InvalidResponse("HTTP 400 Bad Request: x".into()).should_fail_over());
        assert!(!ProviderError::InvalidResponse("json roto".into()).should_fail_over());
    }

    #[test]
    fn each_failure_maps_to_its_own_reason() {
        assert_eq!(ProviderError::QuotaExceeded(String::new()).failover_reason(), Some(FailoverReason::QuotaExceeded));
        assert_eq!(ProviderError::RateLimited.failover_reason(), Some(FailoverReason::RateLimited));
        assert_eq!(ProviderError::ConnectionRefused.failover_reason(), Some(FailoverReason::Unreachable));
        assert_eq!(ProviderError::Timeout.failover_reason(), Some(FailoverReason::Timeout));
        assert_eq!(ProviderError::InvalidResponse("HTTP 502 Bad Gateway: x".into()).failover_reason(), Some(FailoverReason::ServerError));
        assert_eq!(ProviderError::Unauthorized.failover_reason(), None);
        assert_eq!(FailoverReason::Unreachable.as_str(), "unreachable");
    }
}
