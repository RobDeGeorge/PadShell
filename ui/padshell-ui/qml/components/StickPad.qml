import QtQuick

Rectangle {
    id: root
    property string label: ""
    // normalized -1.0 .. 1.0
    property real stickX: 0
    property real stickY: 0

    width: 120
    height: 120
    radius: 8
    color: "#2a2a2a"
    border.color: "#555555"
    border.width: 1

    Rectangle {
        anchors.centerIn: parent
        width: 1
        height: parent.height
        color: "#444444"
    }
    Rectangle {
        anchors.centerIn: parent
        width: parent.width
        height: 1
        color: "#444444"
    }

    Rectangle {
        width: 14
        height: 14
        radius: 7
        color: "#4caf50"
        x: parent.width / 2 - width / 2 + root.stickX * (parent.width / 2 - width)
        y: parent.height / 2 - height / 2 + root.stickY * (parent.height / 2 - height)

        Behavior on x { NumberAnimation { duration: 30 } }
        Behavior on y { NumberAnimation { duration: 30 } }
    }

    Text {
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottomMargin: 4
        text: root.label
        color: "#888888"
        font.pixelSize: 11
    }
}
