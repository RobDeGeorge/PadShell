import QtQuick
import QtQuick.Layouts

// Small battery gauge for the window's top-right corner.
Item {
    id: root
    property int percent: -1          // -1 = unknown
    property bool charging: false
    property bool connected: false
    property int lowThreshold: 20     // colour turns red at/below this

    readonly property bool known: connected && percent >= 0
    readonly property color fill: charging ? "#4fc3f7"
        : percent <= lowThreshold ? "#e53935"
        : percent <= 40 ? "#ffb300" : "#4caf50"

    implicitWidth: row.implicitWidth
    implicitHeight: 24

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        spacing: 6

        Text {
            text: !root.connected ? "No controller"
                : !root.known ? "Battery ?"
                : root.percent + "%" + (root.charging ? " ⚡" : "")
            color: !root.known ? "#777777" : root.fill
            font.pixelSize: 12
            font.bold: root.known && root.percent <= root.lowThreshold && !root.charging
        }

        // Battery body
        Item {
            width: 28; height: 14
            Rectangle {
                id: body
                width: 25; height: 14; radius: 3
                color: "transparent"
                border.color: root.known ? "#bbbbbb" : "#555555"
                border.width: 1
                Rectangle {
                    x: 2; y: 2
                    height: parent.height - 4
                    width: root.known ? Math.max(1, (parent.width - 4) * root.percent / 100) : 0
                    radius: 1
                    color: root.fill
                    Behavior on width { NumberAnimation { duration: 300 } }
                }
            }
            // Terminal nub
            Rectangle {
                x: 25; y: 4; width: 3; height: 6; radius: 1
                color: root.known ? "#bbbbbb" : "#555555"
            }
            // Pulse when low
            SequentialAnimation on opacity {
                running: root.known && !root.charging && root.percent <= root.lowThreshold
                loops: Animation.Infinite
                NumberAnimation { to: 0.35; duration: 700 }
                NumberAnimation { to: 1.0; duration: 700 }
                onRunningChanged: if (!running) parent.opacity = 1.0
            }
        }
    }
}
