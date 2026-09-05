import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.padshell.ui

ApplicationWindow {
    id: window
    visible: true
    width: 1200
    height: 780
    title: "padshell"
    color: "#1e1e1e"

    ConfigBridge {
        id: configBridge
        Component.onCompleted: { load(); load_hypr(""); }
    }

    LiveBridge {
        id: liveBridge
        Component.onCompleted: start()
    }

    HapticsBridge {
        id: hapticsBridge
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        TabBar {
            id: tabBar
            Layout.fillWidth: true

            TabButton { text: "Live Monitor" }
            TabButton { text: "Bindings" }
            TabButton { text: "Haptics" }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabBar.currentIndex

            LiveMonitorView {
                live: liveBridge
            }

            BindingsView {
                config: configBridge
                live: liveBridge
            }

            HapticsView {
                haptics: hapticsBridge
            }
        }
    }
}
