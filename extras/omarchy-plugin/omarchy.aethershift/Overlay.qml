import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Ui
import qs.Commons

Item {
  id: root
  implicitWidth: 320
  implicitHeight: 280

  ColumnLayout {
    anchors.fill: parent
    anchors.margins: 16
    spacing: 12

    Label {
      text: "AetherShift Control"
      font.pixelSize: 18
      font.bold: true
    }

    RowLayout {
      spacing: 8
      Button {
        text: "Windows"
        onClicked: Quickshell.process(["aethershift", "switch", "windows"]).running = true
      }
      Button {
        text: "macOS"
        onClicked: Quickshell.process(["aethershift", "switch", "macos"]).running = true
      }
      Button {
        text: "Hybrid"
        onClicked: Quickshell.process(["aethershift", "switch", "hybrid"]).running = true
      }
      Button {
        text: "Native"
        onClicked: Quickshell.process(["aethershift", "switch", "native"]).running = true
      }
    }

    Label {
      text: "Quick Snap"
      font.pixelSize: 14
      font.bold: true
    }

    RowLayout {
      spacing: 8
      Button {
        text: "Half L"
        onClicked: Quickshell.process(["aethershift", "snap", "half-left"]).running = true
      }
      Button {
        text: "Half R"
        onClicked: Quickshell.process(["aethershift", "snap", "half-right"]).running = true
      }
      Button {
        text: "2/3 L"
        onClicked: Quickshell.process(["aethershift", "snap", "two-thirds-left"]).running = true
      }
      Button {
        text: "Max"
        onClicked: Quickshell.process(["aethershift", "snap", "maximize"]).running = true
      }
    }

    RowLayout {
      spacing: 12
      Button {
        text: "Restore Baseline"
        onClicked: Quickshell.process(["aethershift", "restore"]).running = true
      }
      Button {
        text: "Launch TUI"
        onClicked: Quickshell.process(["xdg-terminal-exec", "aethershift-tui"]).running = true
      }
    }
  }
}
