import QtQuick

Rectangle {
    id: root
    property string label: ""
    property bool active: false
    // Live controller press (Bindings tab shows these on top of "active" = bound)
    property bool pressed: false
    // Currently selected for editing
    property bool selected: false
    // "rect" (default) or "circle" - circle is used for face/dpad/stick-click buttons
    property string shape: "rect"

    width: shape === "circle" ? 44 : 60
    height: shape === "circle" ? 44 : 34
    radius: shape === "circle" ? width / 2 : 6
    color: pressed ? "#ffb300" : active ? "#4caf50" : "#333333"
    border.color: selected ? "#ffd54f" : pressed ? "#ffd54f" : active ? "#6fcf73" : "#555555"
    border.width: selected ? 3 : 1

    Behavior on color { ColorAnimation { duration: 80 } }
    Behavior on border.color { ColorAnimation { duration: 80 } }

    Text {
        anchors.centerIn: parent
        text: root.label
        color: (root.active || root.pressed) ? "#ffffff" : "#aaaaaa"
        font.pixelSize: shape === "circle" ? 10 : 11
        font.bold: root.active
        wrapMode: Text.WordWrap
        width: parent.width - 4
        horizontalAlignment: Text.AlignHCenter
    }
}
