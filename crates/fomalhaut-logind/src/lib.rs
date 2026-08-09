//! Shared non-interactive systemd-logind power backend.

use std::time::Duration;

use fomalhaut_config::{PowerAction as ConfigPowerAction, PowerConfig};
use fomalhaut_web::{
    controller::{PowerControl, PowerControlError},
    protocol::{Capabilities, PowerAction},
};
use zbus::blocking::{Connection, Proxy};

const LOGIN1_DESTINATION: &str = "org.freedesktop.login1";
const LOGIN1_PATH: &str = "/org/freedesktop/login1";
const LOGIN1_INTERFACE: &str = "org.freedesktop.login1.Manager";
const LOGIN1_METHOD_TIMEOUT: Duration = Duration::from_secs(3);

/// Non-interactive logind capability discovery and power execution.
pub struct LogindPowerControl {
    connection: Option<Connection>,
    capabilities: Capabilities,
}

impl LogindPowerControl {
    /// Discovers the intersection of administrator policy and logind `yes` capabilities.
    #[must_use]
    pub fn discover(config: &PowerConfig) -> Self {
        if config.actions().is_empty() {
            return Self {
                connection: None,
                capabilities: Capabilities::disabled(),
            };
        }
        let Ok(connection) = zbus::blocking::connection::Builder::system()
            .and_then(|builder| builder.method_timeout(LOGIN1_METHOD_TIMEOUT).build())
        else {
            return Self {
                connection: None,
                capabilities: Capabilities::disabled(),
            };
        };
        let available = discover_available(&connection, config.actions());
        Self {
            connection: Some(connection),
            capabilities: Capabilities::with_power(&available),
        }
    }
}

impl PowerControl for LogindPowerControl {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    fn request(&mut self, action: PowerAction) -> Result<(), PowerControlError> {
        if !self.capabilities.power().contains(&action) {
            return Err(PowerControlError);
        }
        let connection = self.connection.as_ref().ok_or(PowerControlError)?;
        let proxy = login1_proxy(connection)?;
        let method = match action {
            PowerAction::Poweroff => "PowerOff",
            PowerAction::Reboot => "Reboot",
            PowerAction::Suspend => "Suspend",
        };
        let _: () = proxy
            .call(method, &(false,))
            .map_err(|_| PowerControlError)?;
        Ok(())
    }
}

fn discover_available(
    connection: &Connection,
    configured: &[ConfigPowerAction],
) -> Vec<PowerAction> {
    let Ok(proxy) = login1_proxy(connection) else {
        return Vec::new();
    };
    configured
        .iter()
        .copied()
        .filter(|action| {
            let method = match action {
                ConfigPowerAction::Poweroff => "CanPowerOff",
                ConfigPowerAction::Reboot => "CanReboot",
                ConfigPowerAction::Suspend => "CanSuspend",
            };
            proxy
                .call::<_, _, String>(method, &())
                .is_ok_and(|status| capability_status_allows(&status))
        })
        .map(|action| match action {
            ConfigPowerAction::Poweroff => PowerAction::Poweroff,
            ConfigPowerAction::Reboot => PowerAction::Reboot,
            ConfigPowerAction::Suspend => PowerAction::Suspend,
        })
        .collect()
}

fn capability_status_allows(status: &str) -> bool {
    status == "yes"
}

fn login1_proxy(connection: &Connection) -> Result<Proxy<'_>, PowerControlError> {
    Proxy::new(
        connection,
        LOGIN1_DESTINATION,
        LOGIN1_PATH,
        LOGIN1_INTERFACE,
    )
    .map_err(|_| PowerControlError)
}

#[cfg(test)]
mod tests {
    use fomalhaut_web::{
        controller::{PowerControl, PowerControlError},
        protocol::{Capabilities, PowerAction},
    };

    use super::{LogindPowerControl, capability_status_allows};

    #[test]
    fn unavailable_backend_rejects_every_operation() {
        let mut backend = LogindPowerControl {
            connection: None,
            capabilities: Capabilities::disabled(),
        };
        assert_eq!(
            backend.request(PowerAction::Poweroff),
            Err(PowerControlError)
        );
    }

    #[test]
    fn only_non_interactive_logind_authorization_is_advertised() {
        assert!(capability_status_allows("yes"));
        for status in ["no", "na", "challenge", "unexpected"] {
            assert!(!capability_status_allows(status));
        }
    }
}
