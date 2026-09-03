import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.padshell.ui

Item {
    id: root
    property var live

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 16

        RowLayout {
            spacing: 8
            Rectangle {
                width: 12
                height: 12
                radius: 6
                color: root.live && root.live.connected ? "#4caf50" : "#c0392b"
            }
            Text {
                text: root.live
                    ? (root.live.connected ? "Connected (" + root.live.bus_label + ")" : "Disconnected")
                    : "..."
                color: "#dddddd"
                font.pixelSize: 14
            }
        }

        ControllerDiagram {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: 12
            activeButtons: root.live ? root.live.pressed_buttons : []
            leftStickX: root.live ? root.live.left_stick_x : 0
            leftStickY: root.live ? root.live.left_stick_y : 0
            rightStickX: root.live ? root.live.right_stick_x : 0
            rightStickY: root.live ? root.live.right_stick_y : 0
            l2Analog: root.live ? root.live.l2_analog : 0
            r2Analog: root.live ? root.live.r2_analog : 0
        }

        Item { Layout.fillHeight: true }
    }
}
