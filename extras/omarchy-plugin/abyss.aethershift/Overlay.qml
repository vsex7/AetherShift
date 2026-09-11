import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Ui
import qs.Commons

Item {
  id: root
  implicitWidth: 840
  implicitHeight: 620

  property string currentView: "overview" // "overview", "settings", "cheatsheet", "feedback"
  property var clientWindows: []
  property var activeWorkspaces: []
  property var statusInfo: ({})
  property var currentBindings: []
  property var snapFeedback: ({})

  function open(payloadJson) {
    var payload = ({})
    try { payload = JSON.parse(payloadJson || "{}") } catch(e) { payload = ({}) }
    if (payload.view) {
      root.currentView = payload.view
    } else {
      root.currentView = "overview"
    }

    if (payload.feedback) {
      root.snapFeedback = payload.feedback
      feedbackTimer.restart()
    }

    refreshData()
  }

  function close() {
    Quickshell.process(["omarchy-shell", "shell", "hide", "abyss.aethershift"]).running = true
  }

  Timer {
    id: feedbackTimer
    interval: 800
    repeat: false
    onTriggered: {
      if (root.currentView === "feedback") {
        root.close()
      }
    }
  }

  Process {
    id: clientsProc
    command: ["hyprctl", "clients", "-j"]
    stdout: StderrMode.Ignore
    onExited: function(code) {
      if (code === 0) {
        try {
          root.clientWindows = JSON.parse(clientsProc.readStdout())
        } catch(e) {}
      }
    }
  }

  Process {
    id: statusFetchProc
    command: ["aethershift", "status", "--json"]
    stdout: StderrMode.Ignore
    onExited: function(code) {
      if (code === 0) {
        try {
          root.statusInfo = JSON.parse(statusFetchProc.readStdout())
          bindingsFetchProc.command = ["aethershift", "bindings", root.statusInfo.active_profile || "windows", "--json"]
          bindingsFetchProc.running = true
        } catch(e) {}
      }
    }
  }

  Process {
    id: bindingsFetchProc
    command: ["aethershift", "bindings", "--json"]
    stdout: StderrMode.Ignore
    onExited: function(code) {
      if (code === 0) {
        try {
          var res = JSON.parse(bindingsFetchProc.readStdout())
          root.currentBindings = res.bindings || []
        } catch(e) {}
      }
    }
  }

  function refreshData() {
    clientsProc.running = true
    statusFetchProc.running = true
  }

  // Visual Snap Feedback Box (Shown when in feedback view or when snap feedback is active)
  Rectangle {
    id: snapFeedbackBox
    visible: root.currentView === "feedback" && root.snapFeedback && root.snapFeedback.to
    x: (root.snapFeedback && root.snapFeedback.to) ? root.snapFeedback.to[0] : 0
    y: (root.snapFeedback && root.snapFeedback.to) ? root.snapFeedback.to[1] : 0
    width: (root.snapFeedback && root.snapFeedback.to) ? root.snapFeedback.to[2] : 0
    height: (root.snapFeedback && root.snapFeedback.to) ? root.snapFeedback.to[3] : 0
    color: "#6366f133"
    border.color: "#818cf8"
    border.width: 3
    radius: 12

    Label {
      anchors.centerIn: parent
      text: (root.snapFeedback ? root.snapFeedback.layout : "") + (root.snapFeedback && root.snapFeedback.preview ? " (Preview)" : "")
      font.bold: true
      font.pixelSize: 22
      color: "#ffffff"
    }
  }

  // Main UI Card (Overview, Settings, Cheatsheet)
  Rectangle {
    visible: root.currentView !== "feedback"
    anchors.fill: parent
    color: "#18181be6"
    radius: 12
    border.color: "#3f3f46"
    border.width: 1

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
        highlighted: root.currentView === "overview"
        onClicked: { root.currentView = "overview"; root.refreshData() }
      }

      Button {
        text: "⚙️ Settings"
        highlighted: root.currentView === "settings"
        onClicked: { root.currentView = "settings"; root.refreshData() }
      }

      Button {
        text: "📋 Cheat-Sheet"
        highlighted: root.currentView === "cheatsheet"
        onClicked: { root.currentView = "cheatsheet"; root.refreshData() }
      }

      Button {
        text: "✕ Close"
        onClicked: root.close()
      }
    }

    // View 1: Task Overview
    ScrollView {
      visible: root.currentView === "overview"
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
          model: root.clientWindows
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
                    root.refreshData()
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
                root.close()
              }
            }
          }
        }
      }
    }

    // View 2: Settings Panel
    ColumnLayout {
      visible: root.currentView === "settings"
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
          highlighted: root.statusInfo.active_profile === "windows"
          onClicked: { Quickshell.process(["aethershift", "switch", "windows"]).running = true; root.refreshData() }
        }
        Button {
          text: "🍎 macOS"
          highlighted: root.statusInfo.active_profile === "macos"
          onClicked: { Quickshell.process(["aethershift", "switch", "macos"]).running = true; root.refreshData() }
        }
        Button {
          text: "⚡ Hybrid"
          highlighted: root.statusInfo.active_profile === "hybrid"
          onClicked: { Quickshell.process(["aethershift", "switch", "hybrid"]).running = true; root.refreshData() }
        }
        Button {
          text: "🛡️ Native Baseline"
          highlighted: root.statusInfo.active_profile === "native"
          onClicked: { Quickshell.process(["aethershift", "restore"]).running = true; root.refreshData() }
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
          highlighted: root.statusInfo.window_policy === "tiled"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "tiled"]).running = true; root.refreshData() }
        }
        Button {
          text: "Floating (Always)"
          highlighted: root.statusInfo.window_policy === "floating"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "floating"]).running = true; root.refreshData() }
        }
        Button {
          text: "Follow Profile"
          highlighted: root.statusInfo.window_policy === "follow-profile"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "follow-profile"]).running = true; root.refreshData() }
        }
        Button {
          text: "Omarchy Default"
          highlighted: root.statusInfo.window_policy === "omarchy"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "omarchy"]).running = true; root.refreshData() }
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
        text: "Conflict & Metrics: Overlays: " + (root.statusInfo.overlays_count || 0) +
              " | Skipped: " + (root.statusInfo.skipped_conflicts || 0) +
              " | Forced: " + (root.statusInfo.forced_overrides || 0) +
              " | Latency: " + (root.statusInfo.last_switch_duration_us ? (root.statusInfo.last_switch_duration_us + "µs") : "0µs")
        font.pixelSize: 12
        color: "#a1a1aa"
      }

      Item { Layout.fillHeight: true }
    }

    // View 3: Cheat Sheet HUD
    ScrollView {
      visible: root.currentView === "cheatsheet"
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
          text: "Active Muscular Memory Bindings (" + (root.statusInfo.active_profile || "native").toUpperCase() + ")"
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
          model: (root.currentBindings && root.currentBindings.length > 0) ? root.currentBindings : [
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
    root.close()
  }
}
