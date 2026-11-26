import QtQuick

// Generic controller layout with shared live overlays. The archived ds4.svg
// is retained for a future controller-specific view. Two uses:
//  - Live Monitor: activeButtons = currently pressed buttons (read-only)
//  - Bindings: activeButtons = buttons that already have a binding,
//    clickable so tapping one opens/edits its binding.
Item {
    id: root
    property var activeButtons: []
    // Live presses drawn on top of activeButtons (Bindings tab)
    property var pressedButtons: []
    property string selectedButton: ""
    property bool clickable: false
    property real leftStickX: 0
    property real leftStickY: 0
    property real rightStickX: 0
    property real rightStickY: 0
    property real l2Analog: 0
    property real r2Analog: 0
    signal buttonClicked(string name)

    readonly property real controlScale: 1.12

    readonly property color activeColor: "#4caf50"
    readonly property color pressedColor: "#ffb300"
    readonly property color selectedColor: "#ffd54f"

    function isActive(name) {
        return root.activeButtons.includes(name);
    }
    function isPressed(name) {
        return root.pressedButtons.includes(name);
    }
    function stateColor(name) {
        if (root.isPressed(name)) return root.pressedColor;
        if (root.isActive(name)) return root.activeColor;
        return "transparent";
    }
    function ringColor(name) {
        if (root.selectedButton === name) return root.selectedColor;
        if (root.isPressed(name)) return root.pressedColor;
        if (root.isActive(name)) return root.activeColor;
        return "transparent";
    }

    width: 820
    height: 420

    Image {
        anchors.fill: parent
        source: "../assets/generic.svg"
        sourceSize: Qt.size(820 * 2, 420 * 2) // crisp when scaled
        fillMode: Image.PreserveAspectFit
        smooth: true
    }

    // Translucent highlight that lights up a region of the SVG.
    component Hit: Rectangle {
        id: hit
        scale: root.controlScale
        property string buttonName: ""
        property real cx: 0
        property real cy: 0
        // Hover ring in clickable mode so the user can find targets.
        property bool hovered: mouse.containsMouse
        x: cx - width / 2
        y: cy - height / 2
        color: root.stateColor(buttonName)
        opacity: root.isPressed(buttonName) ? 0.85 : 0.55
        border.color: {
            const c = root.ringColor(buttonName);
            return (c === Qt.color("transparent") && hovered) ? "#ffffff" : c;
        }
        border.width: root.selectedButton === buttonName ? 3 : (hovered ? 1 : 2)
        Behavior on color { ColorAnimation { duration: 60 } }
        Behavior on border.color { ColorAnimation { duration: 60 } }
        MouseArea {
            id: mouse
            anchors.fill: parent
            enabled: root.clickable
            hoverEnabled: root.clickable
            cursorShape: root.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: root.buttonClicked(hit.buttonName)
        }
    }
    component RoundHit: Hit {
        width: 44; height: 44; radius: 22
    }

    // ---- Triggers: analog fill drawn bottom-up, plus press highlight ----
    component TriggerHit: Hit {
        property real value: 0
        width: 84; height: 34; radius: 8
        color: "transparent"
        clip: true
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: parent.height * parent.value
            color: root.activeColor
            opacity: 0.55
            Behavior on height { NumberAnimation { duration: 30 } }
        }
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: root.stateColor(parent.buttonName)
            opacity: 0.35
        }
        Text {
            anchors.top: parent.bottom
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.topMargin: 2
            visible: parent.value > 0.005
            text: parent.value.toFixed(2)
            color: "#bbbbbb"
            font.pixelSize: 9
        }
    }
    TriggerHit { cx: 198; cy: 24; buttonName: "l2"; value: root.l2Analog }
    TriggerHit { cx: 622; cy: 24; buttonName: "r2"; value: root.r2Analog }

    // ---- Shoulders ----
    Hit { cx: 198; cy: 74; width: 104; height: 26; radius: 10; buttonName: "l1" }
    Hit { cx: 622; cy: 74; width: 104; height: 26; radius: 10; buttonName: "r1" }

    // ---- Share / touchpad / options ----
    Hit { cx: 260; cy: 137; width: 18; height: 34; radius: 9; buttonName: "share" }
    Hit { cx: 410; cy: 147; width: 220; height: 108; radius: 14; buttonName: "touchpad_click" }
    Hit { cx: 560; cy: 137; width: 18; height: 34; radius: 9; buttonName: "options" }

    // ---- PS button ----
    Hit { cx: 410; cy: 284; width: 30; height: 30; radius: 15; buttonName: "ps" }

    // ---- D-pad arms ----
    Hit { cx: 136; cy: 142; width: 28; height: 34; radius: 4; buttonName: "dpad_up" }
    Hit { cx: 136; cy: 234; width: 28; height: 34; radius: 4; buttonName: "dpad_down" }
    Hit { cx: 90; cy: 188; width: 34; height: 28; radius: 4; buttonName: "dpad_left" }
    Hit { cx: 182; cy: 188; width: 34; height: 28; radius: 4; buttonName: "dpad_right" }

    // ---- Face buttons ----
    RoundHit { cx: 684; cy: 128; buttonName: "triangle" }
    RoundHit { cx: 684; cy: 248; buttonName: "cross" }
    RoundHit { cx: 624; cy: 188; buttonName: "square" }
    RoundHit { cx: 744; cy: 188; buttonName: "circle" }

    // ---- Sticks: a cap that slides inside the SVG well ----
    component Stick: Item {
        id: stick
        scale: root.controlScale
        property string buttonName: ""
        property real cx: 0
        property real cy: 0
        property real sx: 0
        property real sy: 0
        readonly property real travel: 10
        width: 96; height: 96
        x: cx - width / 2
        y: cy - height / 2

        Rectangle { // cover the SVG resting cap during live movement
            anchors.centerIn: parent
            width: 88; height: 88; radius: 44
            color: "#1c1c1c"
            border.color: "#999999"
        }

        Rectangle { // cap
            width: 70; height: 70; radius: 35
            x: parent.width / 2 - width / 2 + stick.sx * stick.travel
            y: parent.height / 2 - height / 2 + stick.sy * stick.travel
            color: "#858585"
            border.color: {
                const c = root.ringColor(stick.buttonName);
                return c === Qt.color("transparent") ? "#eeeeee" : c;
            }
            border.width: root.selectedButton === stick.buttonName ? 3 : 1
            Behavior on x { NumberAnimation { duration: 30 } }
            Behavior on y { NumberAnimation { duration: 30 } }
            Behavior on border.color { ColorAnimation { duration: 60 } }

            Rectangle { // deflection dot
                anchors.centerIn: parent
                width: 8; height: 8; radius: 4
                visible: Math.abs(stick.sx) > 0.12 || Math.abs(stick.sy) > 0.12
                color: (Math.abs(stick.sx) > 0.12 || Math.abs(stick.sy) > 0.12) ? root.activeColor : "#3a3d44"
            }
            Rectangle { // click highlight
                anchors.fill: parent
                radius: parent.radius
                color: root.stateColor(stick.buttonName)
                opacity: 0.45
                Behavior on color { ColorAnimation { duration: 60 } }
            }
        }
        MouseArea {
            anchors.fill: parent
            enabled: root.clickable
            cursorShape: root.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: root.buttonClicked(stick.buttonName)
        }
    }
    Stick { cx: 275; cy: 326; buttonName: "l3"; sx: root.leftStickX; sy: root.leftStickY }
    Stick { cx: 545; cy: 326; buttonName: "r3"; sx: root.rightStickX; sy: root.rightStickY }

    // Stick readouts under the grips
    Text {
        x: 275 - width / 2; y: 393
        text: "L  " + root.leftStickX.toFixed(2) + ", " + root.leftStickY.toFixed(2)
        color: "#aaaaaa"; font.pixelSize: 12; font.family: "monospace"
    }
    Text {
        x: 545 - width / 2; y: 393
        text: "R  " + root.rightStickX.toFixed(2) + ", " + root.rightStickY.toFixed(2)
        color: "#aaaaaa"; font.pixelSize: 12; font.family: "monospace"
    }
}
