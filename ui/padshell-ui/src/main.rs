mod config_bridge;
mod live_bridge;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    cxx_qt::init_crate!(cxx_qt);
    cxx_qt::init_crate!(cxx_qt_lib);
    cxx_qt::init_crate!(padshell_ui);

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(mut engine) = engine.as_mut() {
        engine
            .as_mut()
            .load(&QUrl::from(&QString::from("qrc:/qt/qml/com/padshell/ui/qml/Main.qml")));
    }

    if let Some(app) = app.as_mut() {
        std::process::exit(app.exec());
    }
}
