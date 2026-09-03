import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.padshell.ui

ApplicationWindow {
    id: window
    visible: true
    width: 900
    height: 640
    title: "padshell"
    color: "#1e1e1e"

    ConfigBridge {
        id: configBridge
        Component.onCompleted: load()
    }

    LiveBridge {
        id: liveBridge
        Component.onCompleted: start()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        TabBar {
            id: tabBar
            Layout.fillWidth: true

            TabButton { text: "Live Monitor" }
            TabButton { text: "Bindings" }
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
            }
        }
    }
}
