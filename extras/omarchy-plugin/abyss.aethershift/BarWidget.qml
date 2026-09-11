import QtQuick
import Quickshell
import Quickshell.Io
import qs.Ui

BarWidget {
  id: root
  moduleName: "abyss.aethershift"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  property string activeProfile: "native"
  property int overlayCount: 0
  property bool daemonOnline: false

  Process {
    id: statusProc
    command: ["aethershift", "status", "--json"]
    stdout: StderrMode.Ignore
    onExited: function(exitCode) {
      if (exitCode === 0) {
        try {
          var data = JSON.parse(statusProc.readStdout())
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
      if (!root.bar) return
      if (mouseButton === Qt.RightButton) {
        // Open the settings panel / overlay
        root.bar.run("omarchy-shell shell summon abyss.aethershift '{\"view\":\"settings\"}'")
      } else {
        // Left click: instant cycle
        root.bar.run("aethershift cycle")
        statusProc.running = true
      }
    }
  }
}
