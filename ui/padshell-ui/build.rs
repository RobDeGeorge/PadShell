use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.padshell.ui").qml_files([
            "qml/Main.qml",
            "qml/BindingsView.qml",
            "qml/LiveMonitorView.qml",
            "qml/HapticsView.qml",
            "qml/components/ButtonIndicator.qml",
            "qml/components/StickPad.qml",
            "qml/components/AnalogBar.qml",
            "qml/components/ControllerDiagram.qml",
            "qml/components/BatteryIndicator.qml",
        ]),
    )
    .file("src/config_bridge.rs")
    .file("src/live_bridge.rs")
    .file("src/haptics_bridge.rs")
    .build();
}
