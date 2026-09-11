import QtQuick
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
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var data = JSON.parse(text || "{}")
          root.statusInfo = data
          root.activeProfile = data.active_profile || "native"
          root.overlayCount = data.overlays_count || 0
          root.daemonOnline = true
        } catch(e) {
          root.daemonOnline = false
        }
      }
    }
  }

  Timer {
    interval: 1500
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: {
      if (!statusProc.running) statusProc.running = true
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
        Row {
          width: parent.width
          spacing: Style.space(8)

          Text {
            text: "⚡ AetherShift"
            font.bold: true
            font.pixelSize: Style.font.title
            color: root.bar ? root.bar.foreground : Color.foreground
          }

          Item {
            width: Math.max(0, parent.width - 160)
            height: 1
          }

          Text {
            text: "v" + (root.statusInfo.version || "0.1.0")
            font.pixelSize: Style.font.caption
            color: "#a1a1aa"
          }
        }

        PanelSeparator {
          width: parent.width
          foreground: root.bar ? root.bar.foreground : Color.foreground
        }

        // Profile Switch Section
        PanelSectionHeader {
          text: "PARADIGM PROFILES"
          foreground: root.bar ? root.bar.foreground : Color.foreground
        }

        Row {
          width: parent.width
          spacing: Style.space(6)
          readonly property real btnW: (width - spacing * 3) / 4

          Button {
            width: parent.btnW
            text: "🪟 Win"
            bordered: true
            active: root.activeProfile === "windows"
            onClicked: {
              Quickshell.process(["aethershift", "switch", "windows"]).running = true
              statusProc.running = true
            }
          }
          Button {
            width: parent.btnW
            text: "🍎 Mac"
            bordered: true
            active: root.activeProfile === "macos"
            onClicked: {
              Quickshell.process(["aethershift", "switch", "macos"]).running = true
              statusProc.running = true
            }
          }
          Button {
            width: parent.btnW
            text: "⚡ Hyb"
            bordered: true
            active: root.activeProfile === "hybrid"
            onClicked: {
              Quickshell.process(["aethershift", "switch", "hybrid"]).running = true
              statusProc.running = true
            }
          }
          Button {
            width: parent.btnW
            text: "🛡️ Nat"
            bordered: true
            active: root.activeProfile === "native"
            onClicked: {
              Quickshell.process(["aethershift", "restore"]).running = true
              statusProc.running = true
            }
          }
        }

        // Window Policy Section
        PanelSectionHeader {
          text: "WINDOW POLICY"
          foreground: root.bar ? root.bar.foreground : Color.foreground
        }

        Row {
          width: parent.width
          spacing: Style.space(6)
          readonly property real btnW: (width - spacing * 2) / 3

          Button {
            width: parent.btnW
            text: "Tiled"
            bordered: true
            active: root.statusInfo.window_policy === "tiled"
            onClicked: {
              Quickshell.process(["aethershift", "window-mode", "set", "tiled"]).running = true
              statusProc.running = true
            }
          }
          Button {
            width: parent.btnW
            text: "Floating"
            bordered: true
            active: root.statusInfo.window_policy === "floating"
            onClicked: {
              Quickshell.process(["aethershift", "window-mode", "set", "floating"]).running = true
              statusProc.running = true
            }
          }
          Button {
            width: parent.btnW
            text: "Follow"
            bordered: true
            active: root.statusInfo.window_policy === "follow-profile"
            onClicked: {
              Quickshell.process(["aethershift", "window-mode", "set", "follow-profile"]).running = true
              statusProc.running = true
            }
          }
        }

        // Quick Snap Row
        PanelSectionHeader {
          text: "QUICK SNAP"
          foreground: root.bar ? root.bar.foreground : Color.foreground
        }

        Row {
          width: parent.width
          spacing: Style.space(6)
          readonly property real btnW: (width - spacing * 3) / 4

          Button {
            width: parent.btnW
            text: "◧ Left"
            bordered: true
            onClicked: {
              Quickshell.process(["aethershift", "snap", "half-left"]).running = true
              root.panelOpen = false
            }
          }
          Button {
            width: parent.btnW
            text: "◨ Right"
            bordered: true
            onClicked: {
              Quickshell.process(["aethershift", "snap", "half-right"]).running = true
              root.panelOpen = false
            }
          }
          Button {
            width: parent.btnW
            text: "🗖 Max"
            bordered: true
            onClicked: {
              Quickshell.process(["aethershift", "snap", "maximize"]).running = true
              root.panelOpen = false
            }
          }
          Button {
            width: parent.btnW
            text: "↺ Reset"
            bordered: true
            onClicked: {
              Quickshell.process(["aethershift", "snap", "restore"]).running = true
              root.panelOpen = false
            }
          }
        }

        PanelSeparator {
          width: parent.width
          foreground: root.bar ? root.bar.foreground : Color.foreground
        }

        // Action Buttons (Full Settings / Overview / HUD)
        Row {
          width: parent.width
          spacing: Style.space(8)
          readonly property real btnW: (width - spacing) / 2

          Button {
            width: parent.btnW
            text: "🪟 Overview"
            bordered: true
            onClicked: {
              root.panelOpen = false
              Quickshell.process(["aethershift", "overview"]).running = true
            }
          }

          Button {
            width: parent.btnW
            text: "📋 Cheat-Sheet"
            bordered: true
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
