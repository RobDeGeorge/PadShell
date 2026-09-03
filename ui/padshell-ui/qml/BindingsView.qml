import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.padshell.ui

Item {
    id: root
    property var config

    property var configData: {
        if (!config || !config.config_json) return {bindings: [], layers: [], macros: []};
        try {
            return JSON.parse(config.config_json);
        } catch (e) {
            return {bindings: [], layers: [], macros: []};
        }
    }

    // Single-button (non-combo) default bindings, for the diagram.
    property var singleBindingButtons: {
        const out = [];
        for (const b of (root.configData.bindings || [])) {
            if (typeof b.input === "string") out.push(b.input);
        }
        return out;
    }

    function bindingForButton(name) {
        for (const b of (root.configData.bindings || [])) {
            if (b.input === name) return b;
        }
        return null;
    }

    function actionJson(type, value) {
        if (type === "key") return JSON.stringify({type: "key", key: value});
        if (type === "key_combo") {
            return JSON.stringify({type: "key_combo", keys: value.split(",").map(s => s.trim()).filter(s => s.length > 0)});
        }
        if (type === "macro") return JSON.stringify({type: "macro", name: value});
        if (type === "mouse_button") return JSON.stringify({type: "mouse_button", button: value});
        return "{}";
    }

    function actionSummary(action) {
        if (!action) return "(unbound)";
        if (action.type === "key") return "key: " + action.key;
        if (action.type === "key_combo") return "keys: " + action.keys.join("+");
        if (action.type === "macro") return "macro: " + action.name;
        if (action.type === "mouse_button") return "mouse: " + action.button;
        return JSON.stringify(action);
    }

    // ---- Inline editor state, opened by clicking a button on the diagram ----
    property string selectedButton: ""
    property bool editorOpen: false

    function openEditorFor(name) {
        root.selectedButton = name;
        root.editorOpen = true;
        const existing = root.bindingForButton(name);
        if (existing) {
            editType.currentIndex = editType.model.indexOf(existing.action.type);
            editValue.text = existing.action.type === "key" ? existing.action.key
                : existing.action.type === "key_combo" ? existing.action.keys.join(",")
                : existing.action.type === "macro" ? existing.action.name
                : existing.action.button;
        } else {
            editType.currentIndex = 0;
            editValue.text = "";
        }
    }

    function saveEditor() {
        const existing = root.bindingForButton(root.selectedButton);
        if (existing) {
            const idx = (root.configData.bindings || []).findIndex(b => b.input === root.selectedButton);
            if (idx >= 0) config.remove_binding(idx);
        }
        const aj = root.actionJson(editType.currentText, editValue.text);
        config.add_binding(root.selectedButton, aj);
        root.editorOpen = false;
    }

    function removeEditorBinding() {
        const idx = (root.configData.bindings || []).findIndex(b => b.input === root.selectedButton);
        if (idx >= 0) config.remove_binding(idx);
        root.editorOpen = false;
    }

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: root.width
            spacing: 20
            anchors.margins: 16
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top

            RowLayout {
                Layout.fillWidth: true
                Text { text: "Bindings"; color: "#dddddd"; font.pixelSize: 18; Layout.fillWidth: true }
                Button { text: "Save"; onClicked: config.save() }
                Button { text: "Reload"; onClicked: config.load() }
            }

            Text {
                visible: config && config.last_error.length > 0
                text: config ? config.last_error : ""
                color: "#e07070"
                font.pixelSize: 12
            }

            Text {
                text: "Click a button below to bind it. Highlighted buttons already have a binding."
                color: "#888888"
                font.pixelSize: 12
            }

            ControllerDiagram {
                id: diagram
                Layout.alignment: Qt.AlignHCenter
                clickable: true
                activeButtons: root.singleBindingButtons
                onButtonClicked: (name) => root.openEditorFor(name)
            }

            // ---- Inline binding editor ----
            GroupBox {
                visible: root.editorOpen
                Layout.fillWidth: true
                title: "Bind: " + root.selectedButton

                RowLayout {
                    anchors.fill: parent
                    ComboBox {
                        id: editType
                        model: ["key", "key_combo", "macro", "mouse_button"]
                        Layout.preferredWidth: 120
                    }
                    ComboBox {
                        id: editKeyPicker
                        visible: editType.currentText === "key"
                        model: config ? config.key_names : []
                        Layout.preferredWidth: 130
                        onActivated: editValue.text = currentText
                    }
                    ComboBox {
                        id: editMousePicker
                        visible: editType.currentText === "mouse_button"
                        model: ["left", "right", "middle"]
                        Layout.preferredWidth: 130
                        onActivated: editValue.text = currentText
                    }
                    TextField {
                        id: editValue
                        visible: editType.currentText === "key_combo" || editType.currentText === "macro"
                        placeholderText: editType.currentText === "key_combo" ? "key,key,..." : "macro name"
                        Layout.fillWidth: true
                    }
                    Item {
                        visible: editType.currentText === "key" || editType.currentText === "mouse_button"
                        Layout.fillWidth: true
                    }
                    Button { text: "Save"; onClicked: root.saveEditor() }
                    Button { text: "Unbind"; onClicked: root.removeEditorBinding() }
                    Button { text: "Cancel"; onClicked: root.editorOpen = false }
                }
            }

            // ---- Combo (chord) bindings: not clickable on the diagram, small manual form ----
            GroupBox {
                title: "Combo bindings (e.g. l1+r1)"
                Layout.fillWidth: true

                ColumnLayout {
                    anchors.fill: parent

                    Repeater {
                        model: (root.configData.bindings || []).filter(b => typeof b.input !== "string")
                        delegate: RowLayout {
                            Layout.fillWidth: true
                            Text { text: modelData.input.combo.join("+"); color: "#dddddd"; Layout.preferredWidth: 120 }
                            Text { text: root.actionSummary(modelData.action); color: "#aaaaaa"; Layout.fillWidth: true }
                            Button {
                                text: "Remove"
                                onClicked: {
                                    const idx = (root.configData.bindings || []).findIndex(b =>
                                        typeof b.input !== "string" && b.input.combo.join("+") === modelData.input.combo.join("+"));
                                    if (idx >= 0) config.remove_binding(idx);
                                }
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        TextField { id: comboInput; placeholderText: "l1+r1"; Layout.preferredWidth: 140 }
                        ComboBox { id: comboType; model: ["key", "key_combo", "macro", "mouse_button"]; Layout.preferredWidth: 110 }
                        TextField { id: comboValue; placeholderText: "value"; Layout.fillWidth: true }
                        Button {
                            text: "Add"
                            onClicked: {
                                if (config.add_binding(comboInput.text, root.actionJson(comboType.currentText, comboValue.text))) {
                                    comboInput.text = "";
                                    comboValue.text = "";
                                }
                            }
                        }
                    }
                }
            }

            // ---- Layers, collapsible ----
            Text { text: "Layers"; color: "#dddddd"; font.pixelSize: 18 }

            Repeater {
                model: root.configData.layers || []
                delegate: ColumnLayout {
                    Layout.fillWidth: true
                    property int layerIndex: index
                    property bool expanded: true

                    RowLayout {
                        Layout.fillWidth: true
                        Rectangle {
                            color: "#2a2a2a"
                            radius: 4
                            Layout.fillWidth: true
                            height: 34
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 6
                                Text {
                                    text: (expanded ? "▾ " : "▸ ") + modelData.name
                                        + "  (activate: " + modelData.activate.button + ", " + modelData.activate.mode + ")"
                                    color: "#dddddd"
                                    Layout.fillWidth: true
                                }
                                Button { text: "Remove layer"; onClicked: config.remove_layer(layerIndex) }
                            }
                            MouseArea {
                                anchors.fill: parent
                                anchors.rightMargin: 110
                                onClicked: expanded = !expanded
                            }
                        }
                    }

                    ColumnLayout {
                        visible: expanded
                        Layout.fillWidth: true
                        Layout.leftMargin: 16

                        Repeater {
                            model: modelData.bindings || []
                            delegate: RowLayout {
                                Layout.fillWidth: true
                                property var linput: (typeof modelData.input === "string") ? modelData.input : modelData.input.combo.join("+")
                                Text { text: linput; color: "#cccccc"; Layout.preferredWidth: 120 }
                                Text { text: root.actionSummary(modelData.action); color: "#999999"; Layout.fillWidth: true }
                                Button { text: "Remove"; onClicked: config.remove_layer_binding(layerIndex, index) }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            TextField { id: layerInput; placeholderText: "button"; Layout.preferredWidth: 120 }
                            ComboBox { id: layerActionType; model: ["key", "key_combo", "macro", "mouse_button"]; Layout.preferredWidth: 110 }
                            TextField { id: layerActionValue; placeholderText: "value"; Layout.fillWidth: true }
                            Button {
                                text: "Add"
                                onClicked: {
                                    const aj = root.actionJson(layerActionType.currentText, layerActionValue.text);
                                    if (config.add_layer_binding(layerIndex, layerInput.text, aj)) {
                                        layerInput.text = "";
                                        layerActionValue.text = "";
                                    }
                                }
                            }
                        }
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: "#333333"; Layout.topMargin: 4 }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                TextField { id: newLayerName; placeholderText: "layer name"; Layout.preferredWidth: 120 }
                TextField { id: newLayerButton; placeholderText: "activate button"; Layout.preferredWidth: 120 }
                ComboBox { id: newLayerMode; model: ["hold", "toggle"]; Layout.preferredWidth: 100 }
                Button {
                    text: "Add layer"
                    onClicked: {
                        if (config.add_layer(newLayerName.text, newLayerButton.text, newLayerMode.currentText === "hold")) {
                            newLayerName.text = "";
                            newLayerButton.text = "";
                        }
                    }
                }
            }

            // ---- Macros ----
            GroupBox {
                title: "Macros"
                Layout.fillWidth: true

                ColumnLayout {
                    anchors.fill: parent

                    Repeater {
                        model: root.configData.macros || []
                        delegate: RowLayout {
                            Layout.fillWidth: true
                            Text { text: modelData.name; color: "#dddddd"; Layout.preferredWidth: 120 }
                            Text { text: (modelData.steps || []).length + " step(s)"; color: "#999999"; Layout.fillWidth: true }
                            Button { text: "Remove"; onClicked: config.remove_macro(index) }
                        }
                    }
                }
            }

            Item { height: 24 }
        }
    }
}
