import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Ui
import qs.Commons

FloatingWindow {
  id: window
  title: "AetherShift Control Center"
  color: "#18181be6"
  implicitWidth: 840
  implicitHeight: 620
  minimumSize: Qt.size(600, 480)

  property var shell: null
  property string currentView: "overview" // "overview", "settings", "cheatsheet"
  property var clientWindows: []
  property var statusInfo: ({})
  property var currentBindings: []

  function open(payloadJson) {
    var payload = ({})
    try { payload = JSON.parse(payloadJson || "{}") } catch(e) { payload = ({}) }
    if (payload.view) {
      window.currentView = payload.view
    } else {
      window.currentView = "overview"
    }
    window.visible = true
    refreshData()
  }

  function close() {
    window.visible = false
    if (window.shell && typeof window.shell.hide === "function") {
      window.shell.hide("abyss.aethershift")
    }
  }

  Process {
    id: clientsProc
    command: ["hyprctl", "clients", "-j"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          window.clientWindows = JSON.parse(text || "[]")
        } catch(e) {}
      }
    }
  }

  Process {
    id: statusFetchProc
    command: ["aethershift", "status", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          window.statusInfo = JSON.parse(text || "{}")
          bindingsFetchProc.command = ["aethershift", "bindings", window.statusInfo.active_profile || "windows", "--json"]
          bindingsFetchProc.running = true
        } catch(e) {}
      }
    }
  }

  Process {
    id: bindingsFetchProc
    command: ["aethershift", "bindings", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var res = JSON.parse(text || "{}")
          window.currentBindings = res.bindings || []
        } catch(e) {}
      }
    }
  }

  function refreshData() {
    if (!clientsProc.running) clientsProc.running = true
    if (!statusFetchProc.running) statusFetchProc.running = true
  }

  Rectangle {
    anchors.fill: parent
    color: "#18181be6"

    // Top Navigation Tabs
    RowLayout {
      id: navBar
      anchors.top: parent.top
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: 16
      spacing: 12

      Label {
        text: "⚡ AetherShift"
        font.bold: true
        font.pixelSize: 20
        color: "#f4f4f5"
      }

      Item { Layout.fillWidth: true }

      Button {
        text: "🪟 Task Overview"
        active: window.currentView === "overview"
        onClicked: { window.currentView = "overview"; window.refreshData() }
      }

      Button {
        text: "⚙️ Settings"
        active: window.currentView === "settings"
        onClicked: { window.currentView = "settings"; window.refreshData() }
      }

      Button {
        text: "📋 Cheat-Sheet"
        active: window.currentView === "cheatsheet"
        onClicked: { window.currentView = "cheatsheet"; window.refreshData() }
      }

      Button {
        text: "✕ Close"
        onClicked: window.close()
      }
    }

    // View 1: Task Overview
    ScrollView {
      visible: window.currentView === "overview"
      anchors.top: navBar.bottom
      anchors.bottom: parent.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: 16
      clip: true

      Flow {
        width: parent.width
        spacing: 16

        Repeater {
          model: window.clientWindows
          delegate: Rectangle {
            width: 250
            height: 140
            radius: 8
            color: "#27272a"
            border.color: mouseArea.containsMouse ? "#6366f1" : "#3f3f46"
            border.width: mouseArea.containsMouse ? 2 : 1

            ColumnLayout {
              anchors.fill: parent
              anchors.margins: 12
              spacing: 6

              RowLayout {
                Layout.fillWidth: true
                Label {
                  text: (modelData.class || "Window").toUpperCase()
                  font.bold: true
                  font.pixelSize: 12
                  color: "#a1a1aa"
                  elide: Text.ElideRight
                  Layout.fillWidth: true
                }
                Button {
                  text: "✕"
                  implicitWidth: 24
                  implicitHeight: 24
                  onClicked: {
                    Quickshell.process(["hyprctl", "dispatch", "closewindow", "address:" + modelData.address]).running = true
                    window.refreshData()
                  }
                }
              }

              Label {
                text: modelData.title || "(Untitled)"
                font.pixelSize: 14
                color: "#fafafa"
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 2
                elide: Text.ElideRight
                Layout.fillWidth: true
              }

              Item { Layout.fillHeight: true }

              RowLayout {
                Layout.fillWidth: true
                Label {
                  text: "Workspace " + (modelData.workspace ? modelData.workspace.id : "?")
                  font.pixelSize: 11
                  color: "#71717a"
                }
                Item { Layout.fillWidth: true }
                Label {
                  text: (modelData.size ? (modelData.size[0] + "x" + modelData.size[1]) : "")
                  font.pixelSize: 10
                  color: "#52525b"
                }
              }
            }

            MouseArea {
              id: mouseArea
              anchors.fill: parent
              hoverEnabled: true
              onClicked: {
                Quickshell.process(["hyprctl", "dispatch", "focuswindow", "address:" + modelData.address]).running = true
                window.close()
              }
            }
          }
        }
      }
    }

    // View 2: Settings Panel
    ColumnLayout {
      visible: window.currentView === "settings"
      anchors.top: navBar.bottom
      anchors.bottom: parent.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: 20
      spacing: 16

      Label {
        text: "Desktop Paradigm Profiles"
        font.bold: true
        font.pixelSize: 16
        color: "#e4e4e7"
      }

      RowLayout {
        spacing: 12
        Button {
          text: "🪟 Windows"
          active: window.statusInfo.active_profile === "windows"
          onClicked: { Quickshell.process(["aethershift", "switch", "windows"]).running = true; window.refreshData() }
        }
        Button {
          text: "🍎 macOS"
          active: window.statusInfo.active_profile === "macos"
          onClicked: { Quickshell.process(["aethershift", "switch", "macos"]).running = true; window.refreshData() }
        }
        Button {
          text: "⚡ Hybrid"
          active: window.statusInfo.active_profile === "hybrid"
          onClicked: { Quickshell.process(["aethershift", "switch", "hybrid"]).running = true; window.refreshData() }
        }
        Button {
          text: "🛡️ Native Baseline"
          active: window.statusInfo.active_profile === "native"
          onClicked: { Quickshell.process(["aethershift", "restore"]).running = true; window.refreshData() }
        }
      }

      Label {
        text: "Window Policy"
        font.bold: true
        font.pixelSize: 16
        color: "#e4e4e7"
      }

      RowLayout {
        spacing: 12
        Button {
          text: "Tiled (Always)"
          active: window.statusInfo.window_policy === "tiled"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "tiled"]).running = true; window.refreshData() }
        }
        Button {
          text: "Floating (Always)"
          active: window.statusInfo.window_policy === "floating"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "floating"]).running = true; window.refreshData() }
        }
        Button {
          text: "Follow Profile"
          active: window.statusInfo.window_policy === "follow-profile"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "follow-profile"]).running = true; window.refreshData() }
        }
        Button {
          text: "Omarchy Default"
          active: window.statusInfo.window_policy === "omarchy"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "omarchy"]).running = true; window.refreshData() }
        }
      }

      Label {
        text: "Precision Window Snap & Layout"
        font.bold: true
        font.pixelSize: 16
        color: "#e4e4e7"
      }

      RowLayout {
        spacing: 8
        Button { text: "Half Left"; onClicked: Quickshell.process(["aethershift", "snap", "half-left"]).running = true }
        Button { text: "Half Right"; onClicked: Quickshell.process(["aethershift", "snap", "half-right"]).running = true }
        Button { text: "2/3 Left"; onClicked: Quickshell.process(["aethershift", "snap", "two-thirds-left"]).running = true }
        Button { text: "1/3 Right"; onClicked: Quickshell.process(["aethershift", "snap", "one-third-right"]).running = true }
        Button { text: "Maximize"; onClicked: Quickshell.process(["aethershift", "snap", "maximize"]).running = true }
        Button { text: "Restore"; onClicked: Quickshell.process(["aethershift", "snap", "restore"]).running = true }
      }

      Label {
        text: "Conflict & Metrics: Overlays: " + (window.statusInfo.overlays_count || 0) +
              " | Skipped: " + (window.statusInfo.skipped_conflicts || 0) +
              " | Forced: " + (window.statusInfo.forced_overrides || 0) +
              " | Latency: " + (window.statusInfo.last_switch_duration_us ? (window.statusInfo.last_switch_duration_us + "µs") : "0µs")
        font.pixelSize: 12
        color: "#a1a1aa"
      }

      Item { Layout.fillHeight: true }
    }

    // View 3: Cheat Sheet HUD
    ScrollView {
      visible: window.currentView === "cheatsheet"
      anchors.top: navBar.bottom
      anchors.bottom: parent.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: 20
      clip: true

      ColumnLayout {
        width: parent.width
        spacing: 12

        Label {
          text: "Active Muscular Memory Bindings (" + (window.statusInfo.active_profile || "native").toUpperCase() + ")"
          font.bold: true
          font.pixelSize: 16
          color: "#6366f1"
        }

        Rectangle {
          Layout.fillWidth: true
          height: 1
          color: "#3f3f46"
        }

        Repeater {
          model: (window.currentBindings && window.currentBindings.length > 0) ? window.currentBindings : [
            { key_combo: "ALT + F4", description: "Close active window" },
            { key_combo: "SUPER + LEFT / RIGHT", description: "Snap window to left/right half screen" },
            { key_combo: "SUPER + UP / DOWN", description: "Maximize window / Restore original geometry" },
            { key_combo: "SUPER + TAB", description: "Summon Task Overview" },
            { key_combo: "ALT + TAB", description: "Cycle windows forward" },
            { key_combo: "SUPER + E", description: "Launch File Manager (Nautilus)" },
            { key_combo: "SUPER + L", description: "Lock System" },
            { key_combo: "SUPER + V", description: "Toggle Clipboard History Manager" },
            { key_combo: "ALT + PRINT", description: "Toggle Screen Recording" },
            { key_combo: "SUPER + PERIOD", description: "Open Emoji Picker" },
            { key_combo: "CTRL + SHIFT + ESCAPE", description: "Open Task Manager (btop)" },
            { key_combo: "SUPER + D", description: "Toggle Desktop Workspace" }
          ]
          delegate: RowLayout {
            Layout.fillWidth: true
            spacing: 20

            Rectangle {
              width: 190
              height: 28
              radius: 6
              color: "#3f3f46"
              Label {
                anchors.centerIn: parent
                text: modelData.key_combo || modelData.key || ""
                font.bold: true
                color: "#fafafa"
              }
            }

            Label {
              text: modelData.description || modelData.desc || modelData.action || ""
              color: "#d4d4d8"
              font.pixelSize: 14
              Layout.fillWidth: true
            }
          }
        }
      }
    }
  }

  Keys.onEscapePressed: {
    window.close()
  }
}
