import QtQuick

Column {
    id: root
    property string label: ""
    // 0.0 .. 1.0
    property real value: 0
    spacing: 4

    Text {
        text: root.label + " " + root.value.toFixed(2)
        color: "#aaaaaa"
        font.pixelSize: 11
    }

    Rectangle {
        width: 160
        height: 14
        radius: 4
        color: "#2a2a2a"
        border.color: "#555555"
        border.width: 1

        Rectangle {
            width: parent.width * root.value
            height: parent.height
            radius: 4
            color: "#4caf50"
            Behavior on width { NumberAnimation { duration: 30 } }
        }
    }
}
