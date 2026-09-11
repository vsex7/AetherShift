import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "abyss.aethershift"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  property string activeProfile: "native"
  property int overlayCount: 0
  property bool daemonOnline: false
  property var statusInfo: ({})
  property bool panelOpen: false

  Process {
    id: statusProc
    command: ["aethershift", "status", "--json"]
    stdout: StderrMode.Ignore
    onExited: function(exitCode) {
      if (exitCode === 0) {
        try {
          var data = JSON.parse(statusProc.readStdout())
          root.statusInfo = data
          root.activeProfile = data.active_profile || "native"
          root.overlayCount = data.overlays_count || 0
          root.daemonOnline = true
        } catch(e) {
          root.daemonOnline = false
        }
      } else {
        root.daemonOnline = false
      }
    }
  }

  Timer {
    interval: 1500
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: {
      statusProc.running = true
    }
  }

  function getProfileBadge() {
    if (!daemonOnline) return "💤 Idle"
    switch (activeProfile) {
      case "windows": return "🪟 Win (" + overlayCount + ")"
      case "macos": return "🍎 Mac (" + overlayCount + ")"
      case "hybrid": return "⚡ Hyb (" + overlayCount + ")"
      case "native": return "🛡️ Nat"
      default: return "✨ " + activeProfile
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.getProfileBadge()
    horizontalMargin: 8

    onPressed: function(mouseButton) {
      if (mouseButton === Qt.RightButton) {
        root.panelOpen = !root.panelOpen
      } else {
        // Left click: instant cycle
        Quickshell.process(["aethershift", "cycle"]).running = true
        statusProc.running = true
      }
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.panelOpen
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(320))
    contentHeight: panel.fittedContentHeight(contentCol.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.panelOpen = false

      Column {
        id: contentCol
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(12)

        // Header
        RowLayout {
          width: parent.width
          spacing: 8

          Text {
            text: "⚡ AetherShift"
            font.bold: true
            font.pixelSize: Style.font.title
            color: root.bar ? root.bar.foreground : "#ffffff"
          }

          Item { Layout.fillWidth: true }

          Text {
            text: "v" + (root.statusInfo.version || "0.1.0")
            font.pixelSize: Style.font.caption
            color: "#a1a1aa"
          }
        }

        Rectangle {
          width: parent.width
          height: 1
          color: "#3f3f46"
        }

        // Profile Switch Section
        Text {
          text: "PARADIGM PROFILES"
          font.pixelSize: Style.font.caption
          font.bold: true
          color: "#a1a1aa"
        }

        RowLayout {
          width: parent.width
          spacing: 6

          Button {
            text: "🪟 Win"
            Layout.fillWidth: true
            highlighted: root.activeProfile === "windows"
            onClicked: {
              Quickshell.process(["aethershift", "switch", "windows"]).running = true
              statusProc.running = true
            }
          }
          Button {
            text: "🍎 Mac"
            Layout.fillWidth: true
            highlighted: root.activeProfile === "macos"
            onClicked: {
              Quickshell.process(["aethershift", "switch", "macos"]).running = true
              statusProc.running = true
            }
          }
          Button {
            text: "⚡ Hyb"
            Layout.fillWidth: true
            highlighted: root.activeProfile === "hybrid"
            onClicked: {
              Quickshell.process(["aethershift", "switch", "hybrid"]).running = true
              statusProc.running = true
            }
          }
          Button {
            text: "🛡️ Nat"
            Layout.fillWidth: true
            highlighted: root.activeProfile === "native"
            onClicked: {
              Quickshell.process(["aethershift", "restore"]).running = true
              statusProc.running = true
            }
          }
        }

        // Window Policy Section
        Text {
          text: "WINDOW POLICY"
          font.pixelSize: Style.font.caption
          font.bold: true
          color: "#a1a1aa"
        }

        RowLayout {
          width: parent.width
          spacing: 6

          Button {
            text: "Tiled"
            Layout.fillWidth: true
            highlighted: root.statusInfo.window_policy === "tiled"
            onClicked: {
              Quickshell.process(["aethershift", "window-mode", "set", "tiled"]).running = true
              statusProc.running = true
            }
          }
          Button {
            text: "Floating"
            Layout.fillWidth: true
            highlighted: root.statusInfo.window_policy === "floating"
            onClicked: {
              Quickshell.process(["aethershift", "window-mode", "set", "floating"]).running = true
              statusProc.running = true
            }
          }
          Button {
            text: "Follow"
            Layout.fillWidth: true
            highlighted: root.statusInfo.window_policy === "follow-profile"
            onClicked: {
              Quickshell.process(["aethershift", "window-mode", "set", "follow-profile"]).running = true
              statusProc.running = true
            }
          }
        }

        // Quick Snap Row
        Text {
          text: "QUICK SNAP"
          font.pixelSize: Style.font.caption
          font.bold: true
          color: "#a1a1aa"
        }

        RowLayout {
          width: parent.width
          spacing: 6

          Button {
            text: "◧ Left"
            Layout.fillWidth: true
            onClicked: {
              Quickshell.process(["aethershift", "snap", "half-left"]).running = true
              root.panelOpen = false
            }
          }
          Button {
            text: "◨ Right"
            Layout.fillWidth: true
            onClicked: {
              Quickshell.process(["aethershift", "snap", "half-right"]).running = true
              root.panelOpen = false
            }
          }
          Button {
            text: "🗖 Max"
            Layout.fillWidth: true
            onClicked: {
              Quickshell.process(["aethershift", "snap", "maximize"]).running = true
              root.panelOpen = false
            }
          }
          Button {
            text: "↺ Reset"
            Layout.fillWidth: true
            onClicked: {
              Quickshell.process(["aethershift", "snap", "restore"]).running = true
              root.panelOpen = false
            }
          }
        }

        Rectangle {
          width: parent.width
          height: 1
          color: "#3f3f46"
        }

        // Action Buttons (Full Settings / Overview / HUD)
        RowLayout {
          width: parent.width
          spacing: 8

          Button {
            text: "🪟 Overview"
            Layout.fillWidth: true
            onClicked: {
              root.panelOpen = false
              Quickshell.process(["aethershift", "overview"]).running = true
            }
          }

          Button {
            text: "📋 Cheat-Sheet"
            Layout.fillWidth: true
            onClicked: {
              root.panelOpen = false
              Quickshell.process(["aethershift", "hud"]).running = true
            }
          }
        }
      }
    }
  }
}
