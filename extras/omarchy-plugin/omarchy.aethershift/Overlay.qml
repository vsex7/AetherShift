import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Ui
import qs.Commons

Item {
  id: root
  implicitWidth: 800
  implicitHeight: 600

  property string currentView: "overview" // "overview", "settings", "cheatsheet"
  property var clientWindows: []
  property var activeWorkspaces: []
  property var statusInfo: ({})

  function open(payloadJson) {
    var payload = ({})
    try { payload = JSON.parse(payloadJson || "{}") } catch(e) { payload = ({}) }
    if (payload.view) {
      root.currentView = payload.view
    } else {
      root.currentView = "overview"
    }
    refreshData()
  }

  function close() {
    root.visible = false
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
        } catch(e) {}
      }
    }
  }

  function refreshData() {
    clientsProc.running = true
    statusFetchProc.running = true
  }

  Rectangle {
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
        onClicked: Quickshell.process(["omarchy-shell", "shell", "dismiss", "omarchy.aethershift"]).running = true
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
            width: 240
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

              Label {
                text: "Workspace " + (modelData.workspace ? modelData.workspace.id : "?")
                font.pixelSize: 11
                color: "#71717a"
              }
            }

            MouseArea {
              id: mouseArea
              anchors.fill: parent
              hoverEnabled: true
              onClicked: {
                Quickshell.process(["hyprctl", "dispatch", "focuswindow", "address:" + modelData.address]).running = true
                Quickshell.process(["omarchy-shell", "shell", "dismiss", "omarchy.aethershift"]).running = true
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
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "tiled"]).running = true; root.refreshData() }
        }
        Button {
          text: "Floating (Always)"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "floating"]).running = true; root.refreshData() }
        }
        Button {
          text: "Follow Profile"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "follow-profile"]).running = true; root.refreshData() }
        }
        Button {
          text: "Omarchy Default"
          onClicked: { Quickshell.process(["aethershift", "window-mode", "set", "omarchy"]).running = true; root.refreshData() }
        }
      }

      Label {
        text: "Precision Window Snap"
        font.bold: true
        font.pixelSize: 16
        color: "#e4e4e7"
      }

      RowLayout {
        spacing: 8
        Button { text: "Half Left"; onClicked: Quickshell.process(["aethershift", "snap", "half-left"]).running = true }
        Button { text: "Half Right"; onClicked: Quickshell.process(["aethershift", "snap", "half-right"]).running = true }
        Button { text: "2/3 Left"; onClicked: Quickshell.process(["aethershift", "snap", "two-thirds-left"]).running = true }
        Button { text: "Maximize"; onClicked: Quickshell.process(["aethershift", "snap", "maximize"]).running = true }
        Button { text: "Restore"; onClicked: Quickshell.process(["aethershift", "snap", "restore"]).running = true }
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
          model: [
            { key: "Alt + F4", desc: "Close active window" },
            { key: "Super + Left / Right", desc: "Snap window to left/right half screen" },
            { key: "Super + Up / Down", desc: "Maximize window / Restore original geometry" },
            { key: "Super + Tab", desc: "Summon Task Overview" },
            { key: "Alt + Tab", desc: "Cycle windows forward" },
            { key: "Super + E", desc: "Launch File Manager (Nautilus)" },
            { key: "Super + L", desc: "Lock System" },
            { key: "Super + V", desc: "Toggle Clipboard History Manager" },
            { key: "Alt + Print", desc: "Toggle Screen Recording" },
            { key: "Super + Period", desc: "Open Emoji Picker" },
            { key: "Ctrl + Shift + Esc", desc: "Open Task Manager (btop)" },
            { key: "Super + D", desc: "Toggle Desktop Workspace" }
          ]
          delegate: RowLayout {
            Layout.fillWidth: true
            spacing: 20

            Rectangle {
              width: 180
              height: 28
              radius: 6
              color: "#3f3f46"
              Label {
                anchors.centerIn: parent
                text: modelData.key
                font.bold: true
                color: "#fafafa"
              }
            }

            Label {
              text: modelData.desc
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
    Quickshell.process(["omarchy-shell", "shell", "dismiss", "omarchy.aethershift"]).running = true
  }
}
