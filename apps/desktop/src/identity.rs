//! Build identity is independent of fixture and remote-development features.
pub const NAME: &str = if cfg!(feature = "dev-build") {
    "ContainerHub (Dev)"
} else {
    "ContainerHub"
};

pub const APP_ID: &str = if cfg!(feature = "dev-build") {
    "dev.containerhub.desktop.dev"
} else {
    "dev.containerhub.desktop"
};

pub const DATA_DIRECTORY: &str = if cfg!(feature = "demo-preview") {
    "ContainerHub Preview"
} else {
    NAME
};

pub fn build_info() -> serde_json::Value {
    serde_json::json!({
        "name": NAME,
        "app_id": APP_ID,
        "version": env!("CARGO_PKG_VERSION"),
        "channel": if cfg!(feature = "dev-build") { "dev" } else { "release" },
        "data_directory": DATA_DIRECTORY,
        "sample_data": containerhub_runtime::DEMO_ENABLED,
        "ui_validation": cfg!(feature = "ui-validation"),
        "remote_development": cfg!(feature = "remote-development"),
    })
}
