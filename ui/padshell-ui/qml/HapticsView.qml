import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.padshell.ui

Item {
    id: root
    property var haptics
    property var config
    property var live

    property var alerts: {
        if (!config || !config.config_json) return [];
        try { return JSON.parse(config.config_json).battery_alerts || []; } catch (e) { return []; }
    }

    property var effectsData: {
        if (!haptics || !haptics.effects_json) return [];
        try {
            return JSON.parse(haptics.effects_json);
        } catch (e) {
            return [];
        }
    }

    Component.onCompleted: {
        if (haptics) {
            haptics.refresh_capabilities();
            haptics.load_effects();
        }
    }

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

    ColumnLayout {
        x: 16
        y: 16
        width: parent.width - 32
        spacing: 20

        RowLayout {
            spacing: 8
            Text { text: "Haptics"; color: "#dddddd"; font.pixelSize: 18; Layout.fillWidth: true }
            Button { text: "Refresh"; onClicked: haptics.refresh_capabilities() }
        }

        Text {
            visible: haptics && haptics.last_error.length > 0
            text: haptics ? haptics.last_error : ""
            color: "#e07070"
            font.pixelSize: 12
        }

        RowLayout {
            spacing: 24

            // ---- Lightbar ----
            GroupBox {
                title: "Lightbar"
                Layout.preferredWidth: 340
                Layout.alignment: Qt.AlignTop

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 10

                    Text {
                        text: haptics && haptics.has_lightbar ? "Ready" : "Not available (no controller connected?)"
                        color: haptics && haptics.has_lightbar ? "#4caf50" : "#c0392b"
                        font.pixelSize: 11
                    }

                    Rectangle {
                        width: 60; height: 60; radius: 8
                        border.color: "#555555"; border.width: 1
                        color: Qt.rgba(redSlider.value / 255, greenSlider.value / 255, blueSlider.value / 255, 1)
                    }

                    RowLayout {
                        Text { text: "R"; color: "#e07070"; Layout.preferredWidth: 14 }
                        Slider { id: redSlider; from: 0; to: 255; value: 0; Layout.fillWidth: true
                            onMoved: haptics.set_lightbar(value, greenSlider.value, blueSlider.value) }
                        Text { text: Math.round(redSlider.value); color: "#aaaaaa"; Layout.preferredWidth: 30 }
                    }
                    RowLayout {
                        Text { text: "G"; color: "#70e070"; Layout.preferredWidth: 14 }
                        Slider { id: greenSlider; from: 0; to: 255; value: 0; Layout.fillWidth: true
                            onMoved: haptics.set_lightbar(redSlider.value, value, blueSlider.value) }
                        Text { text: Math.round(greenSlider.value); color: "#aaaaaa"; Layout.preferredWidth: 30 }
                    }
                    RowLayout {
                        Text { text: "B"; color: "#7070e0"; Layout.preferredWidth: 14 }
                        Slider { id: blueSlider; from: 0; to: 255; value: 0; Layout.fillWidth: true
                            onMoved: haptics.set_lightbar(redSlider.value, greenSlider.value, value) }
                        Text { text: Math.round(blueSlider.value); color: "#aaaaaa"; Layout.preferredWidth: 30 }
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: "#444444"; Layout.topMargin: 4 }

                    Text { text: "Animations"; color: "#888888"; font.pixelSize: 11 }

                    RowLayout {
                        Text { text: "Speed (ms/cycle)"; color: "#aaaaaa"; Layout.preferredWidth: 110 }
                        SpinBox { id: animPeriod; from: 100; to: 10000; value: 2000; stepSize: 100; Layout.fillWidth: true }
                    }

                    RowLayout {
                        spacing: 6
                        Button {
                            text: "Rainbow"
                            onClicked: haptics.start_animation("rainbow", 0, 0, 0, animPeriod.value)
                        }
                        Button {
                            text: "Fade"
                            onClicked: haptics.start_animation("fade", redSlider.value, greenSlider.value, blueSlider.value, animPeriod.value)
                        }
                        Button {
                            text: "Strobe"
                            onClicked: haptics.start_animation("strobe", redSlider.value, greenSlider.value, blueSlider.value, animPeriod.value)
                        }
                    }

                    RowLayout {
                        Text {
                            text: haptics && haptics.is_animating ? "● Animating" : "Stopped"
                            color: haptics && haptics.is_animating ? "#4caf50" : "#888888"
                            font.pixelSize: 11
                            Layout.fillWidth: true
                        }
                        Button {
                            text: "Stop"
                            enabled: haptics && haptics.is_animating
                            onClicked: haptics.stop_animation()
                        }
                    }
                }
            }

            // ---- Rumble ----
            GroupBox {
                title: "Rumble"
                Layout.preferredWidth: 340
                Layout.alignment: Qt.AlignTop

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 10

                    Text {
                        text: haptics && haptics.has_rumble ? "Ready" : "Not available (no controller connected?)"
                        color: haptics && haptics.has_rumble ? "#4caf50" : "#c0392b"
                        font.pixelSize: 11
                    }

                    RowLayout {
                        Text { text: "Weak"; color: "#aaaaaa"; Layout.preferredWidth: 50 }
                        Slider { id: weakSlider; from: 0; to: 65535; value: 32000; Layout.fillWidth: true }
                        Text { text: Math.round(weakSlider.value); color: "#aaaaaa"; Layout.preferredWidth: 50 }
                    }
                    RowLayout {
                        Text { text: "Strong"; color: "#aaaaaa"; Layout.preferredWidth: 50 }
                        Slider { id: strongSlider; from: 0; to: 65535; value: 32000; Layout.fillWidth: true }
                        Text { text: Math.round(strongSlider.value); color: "#aaaaaa"; Layout.preferredWidth: 50 }
                    }
                    RowLayout {
                        Text { text: "Duration (ms)"; color: "#aaaaaa"; Layout.preferredWidth: 90 }
                        SpinBox { id: durationSpin; from: 50; to: 5000; value: 500; stepSize: 50; Layout.fillWidth: true }
                    }
                    Button {
                        text: "Test Rumble"
                        onClicked: haptics.rumble(weakSlider.value, strongSlider.value, durationSpin.value)
                    }
                }
            }
        }

        // ---- Saved effects ----
        GroupBox {
            title: "Saved effects"
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent

                Repeater {
                    model: root.effectsData
                    delegate: RowLayout {
                        Layout.fillWidth: true
                        Rectangle {
                            width: 22; height: 22; radius: 4
                            color: Qt.rgba(modelData.r / 255, modelData.g / 255, modelData.b / 255, 1)
                            border.color: "#555555"; border.width: 1
                        }
                        Text { text: modelData.name; color: "#dddddd"; Layout.preferredWidth: 140 }
                        Text {
                            text: "rumble " + modelData.weak + "/" + modelData.strong + " for " + modelData.duration_ms + "ms"
                            color: "#999999"
                            Layout.fillWidth: true
                        }
                        Button { text: "Play"; onClicked: haptics.play_effect(index) }
                        Button { text: "Remove"; onClicked: haptics.remove_effect(index) }
                    }
                }

                Rectangle { Layout.fillWidth: true; height: 1; color: "#444444" }

                RowLayout {
                    Layout.fillWidth: true
                    TextField { id: effectName; placeholderText: "effect name"; Layout.preferredWidth: 160 }
                    Button {
                        text: "Save current as effect"
                        onClicked: {
                            if (effectName.text.length === 0) return;
                            if (haptics.save_effect(effectName.text, redSlider.value, greenSlider.value, blueSlider.value,
                                    weakSlider.value, strongSlider.value, durationSpin.value)) {
                                effectName.text = "";
                            }
                        }
                    }
                }
            }
        }

        // ---- Battery alerts ----
        GroupBox {
            title: "Battery alerts  (saved to this controller's profile)"
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: 8

                Text {
                    text: "When the battery drops to a threshold (and isn't charging) the daemon switches the lightbar to the alert below; "
                        + "the normal lightbar comes back once it's charged above it. Lowest matching threshold wins."
                        + (root.live && root.live.battery_percent >= 0 ? "   Now: " + root.live.battery_percent + "%" : "")
                    color: "#888888"; font.pixelSize: 11
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }

                Repeater {
                    model: root.alerts
                    delegate: RowLayout {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        spacing: 10
                        Rectangle {
                            width: 22; height: 22; radius: 4
                            color: modelData.lightbar.mode === "off" ? "#222222"
                                : Qt.rgba(modelData.lightbar.r / 255, modelData.lightbar.g / 255, modelData.lightbar.b / 255, 1)
                            border.color: "#555555"; border.width: 1
                        }
                        Text { text: "≤ " + modelData.below_percent + "%"; color: "#dddddd"; font.bold: true; Layout.preferredWidth: 60 }
                        Text {
                            text: modelData.lightbar.mode
                                + (modelData.lightbar.mode === "rainbow" || modelData.lightbar.mode === "fade" || modelData.lightbar.mode === "strobe"
                                    ? " · " + modelData.lightbar.period_ms + " ms" : "")
                                + (modelData.rumble ? " · rumble" : "")
                            color: "#aaaaaa"; Layout.fillWidth: true
                        }
                        Button {
                            text: "Preview"
                            enabled: haptics && haptics.has_lightbar
                            onClicked: {
                                if (modelData.lightbar.mode === "solid") haptics.set_lightbar(modelData.lightbar.r, modelData.lightbar.g, modelData.lightbar.b);
                                else if (modelData.lightbar.mode !== "off")
                                    haptics.start_animation(modelData.lightbar.mode, modelData.lightbar.r, modelData.lightbar.g, modelData.lightbar.b, modelData.lightbar.period_ms);
                            }
                        }
                        Button {
                            text: "Edit"
                            onClicked: {
                                alertEditIndex = index;
                                alertPercent.value = modelData.below_percent;
                                alertMode.currentIndex = alertMode.model.indexOf(modelData.lightbar.mode);
                                alertR.value = modelData.lightbar.r; alertG.value = modelData.lightbar.g; alertB.value = modelData.lightbar.b;
                                alertPeriod.value = modelData.lightbar.period_ms;
                                alertRumble.checked = modelData.rumble;
                            }
                        }
                        Button { text: "Remove"; onClicked: config.remove_battery_alert(index) }
                    }
                }

                Text { visible: root.alerts.length === 0; text: "No alerts yet."; color: "#666666"; font.pixelSize: 11 }

                Rectangle { Layout.fillWidth: true; height: 1; color: "#444444" }

                property int alertEditIndex: -1
                id: alertForm

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Text { text: alertForm.alertEditIndex >= 0 ? "Editing alert:" : "New alert:"; color: "#dddddd"; font.pixelSize: 12 }
                    Text { text: "at or below"; color: "#aaaaaa" }
                    SpinBox { id: alertPercent; from: 1; to: 100; value: 20; stepSize: 5; Layout.preferredWidth: 110 }
                    Text { text: "%"; color: "#aaaaaa" }
                    ComboBox { id: alertMode; model: ["solid", "fade", "strobe", "rainbow", "off"]; currentIndex: 1; Layout.preferredWidth: 110 }
                    Rectangle {
                        width: 22; height: 22; radius: 4
                        color: Qt.rgba(alertR.value / 255, alertG.value / 255, alertB.value / 255, 1)
                        border.color: "#555555"; border.width: 1
                    }
                    Button { text: "Use lightbar color"; onClicked: { alertR.value = redSlider.value; alertG.value = greenSlider.value; alertB.value = blueSlider.value; } }
                    Button { text: "Red"; onClicked: { alertR.value = 255; alertG.value = 0; alertB.value = 0; } }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Text { text: "R"; color: "#e07070" } SpinBox { id: alertR; from: 0; to: 255; value: 255; Layout.preferredWidth: 100 }
                    Text { text: "G"; color: "#70e070" } SpinBox { id: alertG; from: 0; to: 255; value: 0; Layout.preferredWidth: 100 }
                    Text { text: "B"; color: "#7070e0" } SpinBox { id: alertB; from: 0; to: 255; value: 0; Layout.preferredWidth: 100 }
                    Text { text: "Speed (ms)"; color: "#aaaaaa" }
                    SpinBox { id: alertPeriod; from: 100; to: 10000; value: 2000; stepSize: 100; Layout.preferredWidth: 120
                        enabled: alertMode.currentText !== "solid" && alertMode.currentText !== "off" }
                    CheckBox { id: alertRumble; text: "Rumble once"; checked: false }
                    Item { Layout.fillWidth: true }
                    Button {
                        visible: alertForm.alertEditIndex >= 0
                        text: "Cancel"
                        onClicked: alertForm.alertEditIndex = -1
                    }
                    Button {
                        text: alertForm.alertEditIndex >= 0 ? "Update" : "Add alert"
                        highlighted: true
                        onClicked: {
                            if (config.set_battery_alert(alertForm.alertEditIndex, alertPercent.value, alertMode.currentText,
                                    alertR.value, alertG.value, alertB.value, alertPeriod.value, alertRumble.checked)) {
                                alertForm.alertEditIndex = -1;
                                config.save();
                            }
                        }
                    }
                }
                Text {
                    text: "Alerts are saved immediately. The daemon picks them up on its next battery check (within ~15 s)."
                    color: "#666666"; font.pixelSize: 11
                }
            }
        }

        Item { height: 16 }
    }
    }
}
