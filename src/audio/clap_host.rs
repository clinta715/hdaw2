use clack_host::prelude::*;
use clack_extensions::log::{HostLog, HostLogImpl, LogSeverity};
use clack_extensions::gui::{HostGui, HostGuiImpl, GuiSize};

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

impl HostGuiImpl for HdawClapHostShared {
    fn resize_hints_changed(&self) {
        tracing::debug!("CLAP plugin resize hints changed");
    }

    fn request_resize(&self, size: GuiSize) -> Result<(), HostError> {
        tracing::debug!("CLAP plugin requested resize to {}x{}", size.width, size.height);
        Ok(())
    }

    fn request_show(&self) -> Result<(), HostError> {
        tracing::debug!("CLAP plugin requested show");
        Ok(())
    }

    fn request_hide(&self) -> Result<(), HostError> {
        tracing::debug!("CLAP plugin requested hide");
        Ok(())
    }

    fn closed(&self, was_destroyed: bool) {
        tracing::debug!("CLAP plugin GUI closed (destroyed: {})", was_destroyed);
    }
}

impl HostHandlers for HdawClapHost {
    type Shared<'a> = HdawClapHostShared;
    type MainThread<'a> = ();
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<Self>, _shared: &Self::Shared<'_>) {
        builder.register::<HostLog>();
        builder.register::<HostGui>();
    }
}

pub fn make_host_info() -> Result<HostInfo, HostError> {
    HostInfo::new("HDAW", "HDAW", "https://github.com/clinta715/hdaw2", "0.9.0")
        .map_err(HostError::from)
}
