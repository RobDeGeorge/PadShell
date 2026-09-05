import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.padshell.ui

// Game-style controls screen.
//
// Left: the list of things you can do - every bind from your Hyprland
// config (grouped by the section headers in that file), plus raw
// keyboard keys and mouse buttons. Right: the controller. Bind either
// way round: pick an action then press a button on the pad (or click one
// on the diagram), or click a button first and then pick its action.
Item {
    id: root
    property var config
    property var live

    // ------------------------------------------------------------------
    // Config model
    // ------------------------------------------------------------------
    property var configData: {
        if (!config || !config.config_json) return {bindings: [], layers: [], macros: []};
        try { return JSON.parse(config.config_json); } catch (e) { return {bindings: [], layers: [], macros: []}; }
    }

    // -1 = base layer, otherwise index into configData.layers
    property int layerIndex: -1
    property var layerNames: {
        const names = ["Base layer"];
        for (const l of (root.configData.layers || []))
            names.push(l.name + "  (" + l.activate.mode + " " + root.glyph(l.activate.button) + ")");
        return names;
    }
    property var layerBindings: {
        if (root.layerIndex < 0) return root.configData.bindings || [];
        const l = (root.configData.layers || [])[root.layerIndex];
        return l ? (l.bindings || []) : [];
    }

    function bindingForButton(name) {
        for (const b of root.layerBindings) if (b.input === name) return b;
        return null;
    }
    property var boundButtons: {
        const out = [];
        for (const b of root.layerBindings) if (typeof b.input === "string") out.push(b.input);
        return out;
    }

    // ------------------------------------------------------------------
    // Action catalogue: Hyprland binds + raw keys + mouse buttons
    // ------------------------------------------------------------------
    property var hyprActions: {
        if (!config || !config.hypr_actions_json) return [];
        try { return JSON.parse(config.hypr_actions_json); } catch (e) { return []; }
    }

    property var allActions: {
        const out = [];
        const seen = {};
        for (const a of root.hyprActions) {
            // Same action reachable by two shortcuts (arrows and j/k/l) -
            // show it once; the first spelling wins.
            const dedupeKey = a.category + "|" + a.label;
            if (seen[dedupeKey]) continue;
            seen[dedupeKey] = true;
            out.push({
                kind: "hypr", id: "hypr:" + a.id, label: a.label, category: a.category,
                shortcut: a.shortcut, keys: a.keys, reason: a.reason, submap: a.submap
            });
        }
        for (const m of ["left", "right", "middle"]) {
            out.push({kind: "mouse", id: "mouse:" + m, label: "Mouse " + m + " click",
                      category: "Mouse", shortcut: "", keys: null, reason: "", mouse: m});
        }
        const keys = config ? config.key_names : [];
        for (let i = 0; i < keys.length; i++) {
            const k = keys[i];
            out.push({kind: "key", id: "key:" + k, label: "Key " + k, category: "Keyboard keys",
                      shortcut: k, keys: [k], reason: ""});
        }
        return out;
    }

    property string searchText: ""
    property var collapsed: ({})
    function toggleCategory(cat) {
        const c = Object.assign({}, root.collapsed);
        c[cat] = !c[cat];
        root.collapsed = c;
    }

    // Flattened rows for the list: category headers + items.
    property var actionRows: {
        const q = root.searchText.trim().toLowerCase();
        const rows = [];
        let lastCat = null;
        for (const a of root.allActions) {
            if (q.length > 0) {
                const hay = (a.label + " " + a.shortcut + " " + a.category).toLowerCase();
                if (!hay.includes(q)) continue;
            }
            if (a.category !== lastCat) {
                rows.push({isHeader: true, category: a.category, id: "hdr:" + a.category});
                lastCat = a.category;
            }
            if (q.length === 0 && root.collapsed[a.category]) continue;
            rows.push(Object.assign({isHeader: false}, a));
        }
        return rows;
    }

    // ------------------------------------------------------------------
    // Action <-> binding translation
    // ------------------------------------------------------------------
    function actionToJson(a) {
        if (a.kind === "mouse") return JSON.stringify({type: "mouse_button", button: a.mouse});
        if (!a.keys) return "";
        if (a.keys.length === 1) return JSON.stringify({type: "key", key: a.keys[0]});
        return JSON.stringify({type: "key_combo", keys: a.keys});
    }

    function keysSig(keys) { return keys.slice().sort().join("+"); }

    function actionIdForBinding(b) {
        if (!b) return "";
        const act = b.action;
        if (act.type === "mouse_button") return "mouse:" + act.button;
        const sig = act.type === "key" ? act.key : act.type === "key_combo" ? root.keysSig(act.keys) : "";
        if (sig.length > 0) {
            // Only global binds count here: a bare Enter is "Key enter",
            // not "Exit resize mode" from a submap.
            for (const a of root.allActions)
                if (a.kind === "hypr" && a.submap === "" && a.keys && root.keysSig(a.keys) === sig) return a.id;
            if (act.type === "key") return "key:" + act.key;
        }
        return "";
    }

    // Human label for a binding's action - a Hyprland action's name if
    // the keys match one, otherwise the raw keys.
    function describeBinding(b) {
        if (!b) return "";
        const act = b.action;
        if (act.type === "key_combo" || act.type === "key") {
            const sig = act.type === "key" ? act.key : root.keysSig(act.keys);
            for (const a of root.allActions)
                if (a.kind === "hypr" && a.submap === "" && a.keys && root.keysSig(a.keys) === sig) return a.label;
            return act.type === "key" ? "Key " + act.key : act.keys.join(" + ");
        }
        if (act.type === "macro") return "Macro " + act.name;
        if (act.type === "mouse_button") return "Mouse " + act.button + " click";
        return JSON.stringify(act);
    }

    // Buttons (in the current layer) bound to a given action id.
    function buttonsForAction(actionId) {
        const out = [];
        for (const b of root.layerBindings)
            if (typeof b.input === "string" && root.actionIdForBinding(b) === actionId) out.push(b.input);
        return out;
    }

    // ------------------------------------------------------------------
    // Button names / glyphs
    // ------------------------------------------------------------------
    readonly property var buttonOrder: ["cross", "circle", "square", "triangle", "dpad_up", "dpad_down",
        "dpad_left", "dpad_right", "l1", "r1", "l2", "r2", "l3", "r3", "share", "options", "touchpad_click", "ps"]
    readonly property var glyphs: ({
        cross: "✕", circle: "○", square: "◻", triangle: "△",
        dpad_up: "▲", dpad_down: "▼", dpad_left: "◀", dpad_right: "▶",
        l1: "L1", r1: "R1", l2: "L2", r2: "R2", l3: "L3", r3: "R3",
        share: "SHARE", options: "OPTIONS", touchpad_click: "TOUCHPAD", ps: "PS"
    })
    function glyph(name) { return root.glyphs[name] || name; }

    // ------------------------------------------------------------------
    // Assignment state machine
    // ------------------------------------------------------------------
    // Action chosen and waiting for a button (press on pad or click on diagram)
    property var pendingAction: null
    // Button chosen and waiting for an action (click in the list)
    property string selectedButton: ""
    property string flash: ""

    property bool listening: root.pendingAction !== null
    // Buttons held at the moment we started listening are ignored until released.
    property var pressedAtArm: []

    function pickAction(a) {
        if (!a.keys && a.kind !== "mouse") {
            root.flash = "Can't bind \"" + a.label + "\": " + a.reason;
            return;
        }
        if (root.selectedButton.length > 0) {
            root.assign(root.selectedButton, a);
            return;
        }
        root.pendingAction = a;
        root.pressedAtArm = root.live ? root.live.pressed_buttons.slice() : [];
        root.flash = "";
        listFocusSink.forceActiveFocus();
    }

    function pickButton(name) {
        if (root.pendingAction) {
            root.assign(name, root.pendingAction);
            return;
        }
        root.selectedButton = (root.selectedButton === name) ? "" : name;
        root.flash = "";
        listFocusSink.forceActiveFocus();
    }

    function assign(button, action) {
        const aj = root.actionToJson(action);
        if (aj.length === 0) { root.pendingAction = null; root.selectedButton = ""; return; }
        // One button per action: binding it here moves it off any other
        // button in this layer, like a game's controls screen.
        let moved = "";
        for (const other of root.buttonsForAction(action.id)) {
            if (other !== button) {
                config.unbind(root.layerIndex, other);
                moved = other;
            }
        }
        if (config.set_binding(root.layerIndex, button, aj)) {
            root.flash = root.glyph(button) + "  →  " + action.label
                + (moved.length > 0 ? "   (moved from " + root.glyph(moved) + ")" : "");
        }
        root.pendingAction = null;
        root.selectedButton = "";
    }

    function unbindSelected() {
        if (root.selectedButton.length === 0) return;
        config.unbind(root.layerIndex, root.selectedButton);
        root.flash = root.glyph(root.selectedButton) + " unbound";
        root.selectedButton = "";
    }

    function cancel() {
        root.pendingAction = null;
        root.selectedButton = "";
        root.flash = "";
    }

    Connections {
        target: root.live
        function onPressed_buttonsChanged() {
            const now = root.live.pressed_buttons;
            // Forget held-at-arm buttons once they've been released.
            root.pressedAtArm = root.pressedAtArm.filter(b => now.includes(b));
            if (!root.listening) return;
            for (let i = 0; i < now.length; i++) {
                if (!root.pressedAtArm.includes(now[i])) {
                    root.assign(now[i], root.pendingAction);
                    return;
                }
            }
        }
    }

    Shortcut {
        sequence: "Escape"
        enabled: root.visible && (root.listening || root.selectedButton.length > 0)
        onActivated: root.cancel()
    }
    Shortcut {
        sequence: "Delete"
        enabled: root.visible && root.selectedButton.length > 0 && !searchField.activeFocus
        onActivated: root.unbindSelected()
    }
    Item { id: listFocusSink; focus: true }

    // ------------------------------------------------------------------
    // Layout
    // ------------------------------------------------------------------
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        // Header bar
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Text { text: "Controls"; color: "#dddddd"; font.pixelSize: 18 }
            ComboBox {
                id: layerPicker
                model: root.layerNames
                Layout.preferredWidth: 220
                onActivated: { root.layerIndex = currentIndex - 1; root.cancel(); }
            }
            Item { Layout.fillWidth: true }
            Text {
                text: config ? config.hypr_config_path : ""
                color: "#777777"; font.pixelSize: 11
                elide: Text.ElideMiddle
                Layout.maximumWidth: 320
            }
            Button { text: "Reload Hyprland"; onClicked: config.load_hypr("") }
            Rectangle { width: 1; height: 24; color: "#333333" }
            Button { text: "Save"; onClicked: config.save() }
            Button { text: "Reload"; onClicked: { config.load(); root.cancel(); } }
        }

        Text {
            visible: config && (config.last_error.length > 0 || config.hypr_error.length > 0)
            text: config ? (config.last_error.length > 0 ? config.last_error : config.hypr_error) : ""
            color: "#e07070"; font.pixelSize: 12
            Layout.fillWidth: true
            wrapMode: Text.Wrap
        }

        // Status banner - the "press a button" prompt
        Rectangle {
            Layout.fillWidth: true
            height: 40
            radius: 6
            color: root.listening ? "#3d3416" : root.selectedButton.length > 0 ? "#16303d" : "#232323"
            border.color: root.listening ? "#ffb300" : root.selectedButton.length > 0 ? "#4fc3f7" : "#333333"
            border.width: 1
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 8
                spacing: 10
                Rectangle {
                    visible: root.listening
                    width: 10; height: 10; radius: 5; color: "#ffb300"
                    SequentialAnimation on opacity {
                        running: root.listening; loops: Animation.Infinite
                        NumberAnimation { to: 0.2; duration: 500 }
                        NumberAnimation { to: 1.0; duration: 500 }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    color: "#eeeeee"; font.pixelSize: 13
                    elide: Text.ElideRight
                    text: root.listening
                        ? "Press a button on the controller for \"" + root.pendingAction.label + "\"  (or click one below · Esc to cancel)"
                        : root.selectedButton.length > 0
                            ? root.glyph(root.selectedButton) + "  ·  " + (root.bindingForButton(root.selectedButton)
                                ? "currently: " + root.describeBinding(root.bindingForButton(root.selectedButton))
                                : "unbound") + "  ·  pick an action from the list"
                            : root.flash.length > 0 ? root.flash
                            : (root.live && root.live.connected
                                ? "Pick an action, then press a controller button. Or click a button first."
                                : "Controller not connected - click buttons on the diagram to bind them.")
                }
                Button {
                    visible: root.selectedButton.length > 0 && root.bindingForButton(root.selectedButton) !== null
                    text: "Unbind"; onClicked: root.unbindSelected()
                }
                Button { visible: root.listening || root.selectedButton.length > 0; text: "Cancel"; onClicked: root.cancel() }
            }
        }

        // Main split
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 12

            // ---- Actions list ----
            Rectangle {
                Layout.preferredWidth: 400
                Layout.fillHeight: true
                color: "#1a1a1a"; radius: 6; border.color: "#333333"

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 6

                    TextField {
                        id: searchField
                        Layout.fillWidth: true
                        placeholderText: "Search actions…  (e.g. workspace, kitty, close)"
                        onTextChanged: root.searchText = text
                    }

                    ListView {
                        id: actionList
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        model: root.actionRows
                        spacing: 1
                        ScrollBar.vertical: ScrollBar {}

                        delegate: Item {
                            id: row
                            required property var modelData
                            required property int index
                            width: actionList.width
                            height: modelData.isHeader ? 30 : 34

                            // Category header
                            Rectangle {
                                visible: row.modelData.isHeader
                                anchors.fill: parent
                                color: "#262626"; radius: 4
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 8
                                    Text {
                                        text: (root.collapsed[row.modelData.category] && root.searchText.length === 0 ? "▸ " : "▾ ") + row.modelData.category
                                        color: "#bbbbbb"; font.pixelSize: 12; font.bold: true
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                    }
                                }
                                MouseArea { anchors.fill: parent; onClicked: root.toggleCategory(row.modelData.category) }
                            }

                            // Action row
                            Rectangle {
                                visible: !row.modelData.isHeader
                                anchors.fill: parent
                                property bool bindable: row.modelData.isHeader ? false : (row.modelData.keys !== null || row.modelData.kind === "mouse")
                                property bool isPending: root.pendingAction !== null && !row.modelData.isHeader && root.pendingAction.id === row.modelData.id
                                property var bound: row.modelData.isHeader ? [] : root.buttonsForAction(row.modelData.id)
                                color: isPending ? "#3d3416" : rowHover.containsMouse ? "#2b2b2b" : "transparent"
                                radius: 4
                                opacity: bindable ? 1.0 : 0.45

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 10
                                    anchors.rightMargin: 8
                                    spacing: 8
                                    Text {
                                        text: row.modelData.isHeader ? "" : row.modelData.label
                                        color: "#e6e6e6"; font.pixelSize: 13
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                    }
                                    Text {
                                        text: row.modelData.isHeader ? "" : row.modelData.shortcut
                                        color: "#777777"; font.pixelSize: 11
                                        font.family: "monospace"
                                    }
                                    // Controller glyph chips for buttons already bound to this action
                                    Repeater {
                                        model: parent.parent.bound
                                        delegate: Rectangle {
                                            required property var modelData
                                            width: chip.implicitWidth + 12; height: 22; radius: 11
                                            color: "#2e7d32"; border.color: "#6fcf73"
                                            Text {
                                                id: chip
                                                anchors.centerIn: parent
                                                text: root.glyph(modelData)
                                                color: "white"; font.pixelSize: 11; font.bold: true
                                            }
                                        }
                                    }
                                }
                                MouseArea {
                                    id: rowHover
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: parent.bindable ? Qt.PointingHandCursor : Qt.ArrowCursor
                                    onClicked: root.pickAction(row.modelData)
                                }
                                ToolTip.visible: rowHover.containsMouse && !parent.bindable && !row.modelData.isHeader
                                ToolTip.text: row.modelData.isHeader ? "" : row.modelData.reason
                                ToolTip.delay: 400
                            }
                        }
                    }
                }
            }

            // ---- Controller + assignments ----
            ColumnLayout {
                id: rightCol
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 8

                Item {
                    id: diagramBox
                    Layout.fillWidth: true
                    // Shrink the diagram so the assignment table below always
                    // keeps at least ~5 rows visible.
                    property real s: Math.max(0.35, Math.min(0.85, width / 820, (rightCol.height - 170) / 420))
                    Layout.preferredHeight: 420 * s
                    ControllerDiagram {
                        id: diagram
                        x: (diagramBox.width - 820 * diagramBox.s) / 2
                        scale: diagramBox.s
                        transformOrigin: Item.TopLeft
                        clickable: true
                        activeButtons: root.boundButtons
                        pressedButtons: root.live ? root.live.pressed_buttons : []
                        selectedButton: root.selectedButton
                        leftStickX: root.live ? root.live.left_stick_x : 0
                        leftStickY: root.live ? root.live.left_stick_y : 0
                        rightStickX: root.live ? root.live.right_stick_x : 0
                        rightStickY: root.live ? root.live.right_stick_y : 0
                        l2Analog: root.live ? root.live.l2_analog : 0
                        r2Analog: root.live ? root.live.r2_analog : 0
                        onButtonClicked: (name) => root.pickButton(name)
                    }
                }

                // Assignment table, two columns like a game's controls screen
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    color: "#1a1a1a"; radius: 6; border.color: "#333333"

                    GridView {
                        id: assignGrid
                        anchors.fill: parent
                        anchors.margins: 8
                        clip: true
                        cellWidth: Math.floor(width / 2)
                        cellHeight: 30
                        model: root.buttonOrder
                        ScrollBar.vertical: ScrollBar {}

                        delegate: Item {
                            id: cell
                            required property string modelData
                            width: assignGrid.cellWidth
                            height: assignGrid.cellHeight
                            property var binding: root.bindingForButton(modelData)
                            property bool isSelected: root.selectedButton === modelData
                            property bool isPressed: root.live && root.live.pressed_buttons.includes(modelData)

                            Rectangle {
                                anchors.fill: parent
                                anchors.rightMargin: 4
                                anchors.bottomMargin: 2
                                radius: 4
                                color: cell.isSelected ? "#16303d" : cellHover.containsMouse ? "#2b2b2b" : "transparent"
                                border.color: cell.isSelected ? "#4fc3f7" : "transparent"

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 6
                                    anchors.rightMargin: 6
                                    spacing: 8
                                    Rectangle {
                                        width: 64; height: 22; radius: 11
                                        color: cell.isPressed ? "#ffb300" : cell.binding ? "#2e7d32" : "#333333"
                                        border.color: cell.isPressed ? "#ffd54f" : cell.binding ? "#6fcf73" : "#555555"
                                        Text {
                                            anchors.centerIn: parent
                                            text: root.glyph(cell.modelData)
                                            color: "white"; font.pixelSize: 11; font.bold: true
                                        }
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: cell.binding ? root.describeBinding(cell.binding) : "—"
                                        color: cell.binding ? "#e6e6e6" : "#666666"
                                        font.pixelSize: 12
                                        elide: Text.ElideRight
                                    }
                                }
                                MouseArea {
                                    id: cellHover
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: root.pickButton(cell.modelData)
                                }
                            }
                        }
                    }
                }
            }
        }

        // ---- Advanced: combos, layers, macros ----
        Rectangle {
            Layout.fillWidth: true
            height: 30
            color: "#232323"; radius: 4
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                Text {
                    text: (advanced.visible ? "▾ " : "▸ ") + "Advanced: chords, layers, macros"
                    color: "#bbbbbb"; font.pixelSize: 12; font.bold: true
                    Layout.fillWidth: true
                }
            }
            MouseArea { anchors.fill: parent; onClicked: advanced.visible = !advanced.visible }
        }

        ScrollView {
            id: advanced
            visible: false
            Layout.fillWidth: true
            Layout.preferredHeight: 260
            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                id: adv
                width: advanced.availableWidth
                spacing: 12

                function actionJson(type, value) {
                    if (type === "key") return JSON.stringify({type: "key", key: value});
                    if (type === "key_combo")
                        return JSON.stringify({type: "key_combo", keys: value.split(",").map(s => s.trim()).filter(s => s.length > 0)});
                    if (type === "macro") return JSON.stringify({type: "macro", name: value});
                    if (type === "mouse_button") return JSON.stringify({type: "mouse_button", button: value});
                    return "{}";
                }

                // Chords
                GroupBox {
                    title: "Chord bindings (e.g. l1+r1) - base layer"
                    Layout.fillWidth: true
                    ColumnLayout {
                        anchors.fill: parent
                        Repeater {
                            model: (root.configData.bindings || []).filter(b => typeof b.input !== "string")
                            delegate: RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                Text { text: modelData.input.combo.map(b => root.glyph(b)).join(" + "); color: "#dddddd"; Layout.preferredWidth: 140 }
                                Text { text: root.describeBinding(modelData); color: "#aaaaaa"; Layout.fillWidth: true }
                                Button { text: "Remove"; onClicked: config.unbind(-1, modelData.input.combo.join("+")) }
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            TextField { id: comboInput; placeholderText: "l1+r1"; Layout.preferredWidth: 140 }
                            ComboBox { id: comboType; model: ["key", "key_combo", "macro", "mouse_button"]; Layout.preferredWidth: 120 }
                            TextField { id: comboValue; placeholderText: comboType.currentText === "key_combo" ? "super,shift,q" : "value"; Layout.fillWidth: true }
                            Button {
                                text: "Add"
                                onClicked: {
                                    if (config.set_binding(-1, comboInput.text, adv.actionJson(comboType.currentText, comboValue.text))) {
                                        comboInput.text = ""; comboValue.text = "";
                                    }
                                }
                            }
                        }
                    }
                }

                // Layers
                GroupBox {
                    title: "Layers (hold/toggle a button to switch to a second set of bindings)"
                    Layout.fillWidth: true
                    ColumnLayout {
                        anchors.fill: parent
                        Repeater {
                            model: root.configData.layers || []
                            delegate: RowLayout {
                                required property var modelData
                                required property int index
                                Layout.fillWidth: true
                                Text {
                                    text: modelData.name + "  ·  " + modelData.activate.mode + " " + root.glyph(modelData.activate.button)
                                        + "  ·  " + (modelData.bindings || []).length + " binding(s)"
                                        + (modelData.stick_mouse ? "  ·  mouse on " + modelData.stick_mouse.stick + " stick" : "")
                                    color: "#dddddd"; Layout.fillWidth: true
                                }
                                Button { text: "Edit bindings"; onClicked: { root.layerIndex = index; layerPicker.currentIndex = index + 1; root.cancel(); } }
                                Button { text: "Remove"; onClicked: { if (root.layerIndex >= index) { root.layerIndex = -1; layerPicker.currentIndex = 0; } config.remove_layer(index); } }
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            TextField { id: newLayerName; placeholderText: "layer name"; Layout.preferredWidth: 140 }
                            ComboBox { id: newLayerButton; model: config ? config.button_names : []; Layout.preferredWidth: 150 }
                            ComboBox { id: newLayerMode; model: ["hold", "toggle"]; Layout.preferredWidth: 100 }
                            Button {
                                text: "Add layer"
                                onClicked: {
                                    if (config.add_layer(newLayerName.text, newLayerButton.currentText, newLayerMode.currentText === "hold"))
                                        newLayerName.text = "";
                                }
                            }
                        }
                    }
                }

                // Macros
                GroupBox {
                    title: "Macros"
                    Layout.fillWidth: true
                    ColumnLayout {
                        anchors.fill: parent
                        Repeater {
                            model: root.configData.macros || []
                            delegate: RowLayout {
                                required property var modelData
                                required property int index
                                Layout.fillWidth: true
                                Text { text: modelData.name; color: "#dddddd"; Layout.preferredWidth: 140 }
                                Text { text: (modelData.steps || []).length + " step(s)"; color: "#999999"; Layout.fillWidth: true }
                                Button { text: "Remove"; onClicked: config.remove_macro(index) }
                            }
                        }
                        Text { text: "Macros are edited in the TOML file for now; bind one via a chord above with type \"macro\"."; color: "#777777"; font.pixelSize: 11 }
                    }
                }
            }
        }
    }
}
