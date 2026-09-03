import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.padshell.ui

Item {
    id: root
    property var haptics

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

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
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

        Item { Layout.fillHeight: true }
    }
}
