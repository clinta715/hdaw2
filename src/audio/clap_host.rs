use clack_host::prelude::*;
use clack_extensions::log::{HostLog, HostLogImpl, LogSeverity};

pub struct HdawClapHost;

pub struct HdawClapHostShared;

impl<'a> SharedHandler<'a> for HdawClapHostShared {
    fn request_restart(&self) {
        tracing::debug!("CLAP plugin requested restart");
    }

    fn request_process(&self) {
        tracing::debug!("CLAP plugin requested process");
    }

    fn request_callback(&self) {
        tracing::debug!("CLAP plugin requested callback");
    }
}

impl HostLogImpl for HdawClapHostShared {
    fn log(&self, severity: LogSeverity, message: &str) {
        match severity {
            LogSeverity::Debug => tracing::debug!("CLAP: {}", message),
            LogSeverity::Info => tracing::info!("CLAP: {}", message),
            LogSeverity::Warning => tracing::warn!("CLAP: {}", message),
            LogSeverity::Error => tracing::error!("CLAP: {}", message),
            LogSeverity::Fatal => tracing::error!("CLAP FATAL: {}", message),
            _ => tracing::trace!("CLAP: {}", message),
        }
    }
}

impl HostHandlers for HdawClapHost {
    type Shared<'a> = HdawClapHostShared;
    type MainThread<'a> = ();
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<Self>, _shared: &Self::Shared<'_>) {
        builder.register::<HostLog>();
    }
}

pub fn make_host_info() -> Result<HostInfo, HostError> {
    HostInfo::new("HDAW", "HDAW", "https://github.com/clinta715/hdaw2", "0.1.0")
        .map_err(|e| HostError::from(e))
}