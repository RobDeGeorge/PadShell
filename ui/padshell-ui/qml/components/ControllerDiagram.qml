import QtQuick
import QtQuick.Layouts

// Reusable DS4-shaped button layout. Two uses:
//  - Live Monitor: activeButtons = currently pressed buttons (read-only)
//  - Bindings: activeButtons = buttons that already have a binding,
//    clickable so tapping one opens/edits its binding.
Item {
    id: root
    property var activeButtons: []
    property bool clickable: false
    property real leftStickX: 0
    property real leftStickY: 0
    property real rightStickX: 0
    property real rightStickY: 0
    property real l2Analog: 0
    property real r2Analog: 0
    signal buttonClicked(string name)

    function isActive(name) {
        return root.activeButtons.includes(name);
    }

    width: 820
    height: 420

    component Btn: ButtonIndicator {
        id: btn
        property string buttonName: ""
        shape: "circle"
        active: root.isActive(buttonName)
        MouseArea {
            anchors.fill: parent
            enabled: root.clickable
            cursorShape: root.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: root.buttonClicked(btn.buttonName)
        }
    }

    // ---- Shoulders / triggers, top row ----
    AnalogBar { x: 20; y: 0; label: "L2"; value: root.l2Analog }
    AnalogBar { x: root.width - 20 - 160; y: 0; label: "R2"; value: root.r2Analog }
    Btn { x: 20; y: 46; buttonName: "l1"; shape: "rect"; label: "L1" }
    Btn { x: root.width - 20 - 60; y: 46; buttonName: "r1"; shape: "rect"; label: "R1" }

    // ---- Share / touchpad / options row ----
    Btn { x: 220; y: 60; buttonName: "share"; shape: "rect"; label: "SHARE" }
    Btn {
        x: root.width / 2 - 40; y: 46
        buttonName: "touchpad_click"; shape: "rect"; width: 80; height: 30
        label: "TOUCHPAD"
    }
    Btn { x: root.width - 220 - 60; y: 60; buttonName: "options"; shape: "rect"; label: "OPTIONS" }

    // ---- PS button, center ----
    Btn { x: root.width / 2 - 22; y: 130; buttonName: "ps"; label: "PS" }

    // ---- D-pad diamond, left side ----
    Btn { x: 165 - 22; y: 130; buttonName: "dpad_up"; label: "▲" }
    Btn { x: 165 - 22; y: 130 + 88; buttonName: "dpad_down"; label: "▼" }
    Btn { x: 165 - 22 - 44; y: 130 + 44; buttonName: "dpad_left"; label: "◀" }
    Btn { x: 165 - 22 + 44; y: 130 + 44; buttonName: "dpad_right"; label: "▶" }

    // ---- Face buttons diamond, right side ----
    Btn { x: root.width - 165 - 22; y: 130; buttonName: "triangle"; label: "△" }
    Btn { x: root.width - 165 - 22; y: 130 + 88; buttonName: "cross"; label: "✕" }
    Btn { x: root.width - 165 - 22 - 44; y: 130 + 44; buttonName: "square"; label: "◻" }
    Btn { x: root.width - 165 - 22 + 44; y: 130 + 44; buttonName: "circle"; label: "○" }

    // ---- Sticks, bottom row ----
    StickPad {
        x: 100; y: 280
        label: "L3"
        stickX: root.leftStickX
        stickY: root.leftStickY
        border.color: root.isActive("l3") ? "#4caf50" : "#555555"
        MouseArea {
            anchors.fill: parent
            enabled: root.clickable
            cursorShape: root.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: root.buttonClicked("l3")
        }
    }
    StickPad {
        x: root.width - 100 - 120; y: 280
        label: "R3"
        stickX: root.rightStickX
        stickY: root.rightStickY
        border.color: root.isActive("r3") ? "#4caf50" : "#555555"
        MouseArea {
            anchors.fill: parent
            enabled: root.clickable
            cursorShape: root.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: root.buttonClicked("r3")
        }
    }
}
