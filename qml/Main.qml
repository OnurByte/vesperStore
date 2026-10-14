import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQml

ApplicationWindow {
    id: window

    width: 1080
    height: 720
    minimumWidth: 760
    minimumHeight: 520
    visible: true
    title: qsTr("Vesper Store")
    color: palette.window

    property int pageIndex: 0
    property var catalogStatus: parseJson(StoreCatalogStatusJson, ({ schemaVersion: 1, available: false, path: "" }))
    property var sourceStatus: parseJson(StoreSourcesJson, ({ nixpkgs: { enabled: true, default: true }, flathub: { enabled: false, default: false } }))
    property var searchStatus: ({ available: false, results: [] })
    property var actionStatus: ({})

    function parseJson(value, fallback) {
        try {
            return JSON.parse(value || "{}");
        } catch (error) {
            return fallback;
        }
    }

    function scheduleSearch() {
        if (window.catalogStatus.available)
            searchDebounce.restart();
    }

    Component.onCompleted: {
        const initialSearch = StoreInitialSearch || ""
        if (initialSearch.length === 0)
            return
        window.pageIndex = 2
        searchField.text = initialSearch
        searchField.forceActiveFocus()
    }

    Connections {
        target: StoreSearch

        function onValueChanged(key, value) {
            if (key === "json")
                window.searchStatus = window.parseJson(value, ({ available: false, results: [] }));
        }
    }

    Connections {
        target: StoreActions

        function onValueChanged(key, value) {
            if (key === "json")
                window.actionStatus = window.parseJson(value, ({}));
        }
    }

    Timer {
        id: searchDebounce
        interval: 250
        repeat: false
        onTriggered: StoreSearch.query = searchField.text
    }

    component PageTitle: Label {
        font.pixelSize: 28
        font.weight: Font.DemiBold
        color: window.palette.text
    }

    component MutedText: Label {
        color: window.palette.placeholderText
        wrapMode: Text.WordWrap
    }

    component SourceCard: Frame {
        id: card
        required property string sourceName
        required property string detail
        required property bool enabled
        required property bool primary

        Layout.fillWidth: true
        padding: 18

        RowLayout {
            anchors.fill: parent
            spacing: 16

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4

                Label {
                    text: card.sourceName
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }

                MutedText {
                    Layout.fillWidth: true
                    text: card.detail
                }
            }

            Label {
                text: card.primary ? qsTr("Default") : (card.enabled ? qsTr("On") : qsTr("Off"))
                font.weight: Font.Medium
            }
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Pane {
            Layout.preferredWidth: 236
            Layout.fillHeight: true
            padding: 18

            ColumnLayout {
                anchors.fill: parent
                spacing: 8

                Label {
                    text: qsTr("Vesper Store")
                    font.pixelSize: 21
                    font.weight: Font.DemiBold
                    Layout.bottomMargin: 12
                }

                Repeater {
                    model: [
                        { label: qsTr("Discover"), index: 0 },
                        { label: qsTr("Categories"), index: 1 },
                        { label: qsTr("Search"), index: 2 },
                        { label: qsTr("Sources"), index: 3 }
                    ]

                    delegate: Button {
                        required property var modelData
                        Layout.fillWidth: true
                        text: modelData.label
                        flat: window.pageIndex !== modelData.index
                        highlighted: window.pageIndex === modelData.index
                        onClicked: window.pageIndex = modelData.index
                    }
                }

                Item { Layout.fillHeight: true }

                MutedText {
                    Layout.fillWidth: true
                    text: window.catalogStatus.available
                        ? qsTr("Local catalogue ready")
                        : qsTr("Local catalogue unavailable")
                }
            }
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            color: window.palette.mid
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: window.pageIndex

            ScrollView {
                clip: true

                ColumnLayout {
                    width: parent.width
                    spacing: 18
                    anchors.margins: 32

                    PageTitle { text: qsTr("Discover") }

                    TextField {
                        Layout.fillWidth: true
                        placeholderText: qsTr("Search applications")
                        enabled: window.catalogStatus.available
                        onAccepted: {
                            window.pageIndex = 2;
                            searchField.text = text;
                            searchField.forceActiveFocus();
                        }
                    }

                    Frame {
                        Layout.fillWidth: true
                        padding: 24

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 8

                            Label {
                                text: window.catalogStatus.available
                                    ? qsTr("Catalogue ready")
                                    : qsTr("Catalogue not built yet")
                                font.pixelSize: 18
                                font.weight: Font.DemiBold
                            }

                            MutedText {
                                Layout.fillWidth: true
                                text: window.catalogStatus.available
                                    ? qsTr("Discovery data is available locally.")
                                    : qsTr("Vesper Store will stay offline-first. Search becomes available when the locked Nixpkgs catalogue is connected.")
                            }
                        }
                    }
                }
            }

            ScrollView {
                clip: true

                ColumnLayout {
                    width: parent.width
                    spacing: 18
                    anchors.margins: 32

                    PageTitle { text: qsTr("Categories") }
                    MutedText {
                        Layout.fillWidth: true
                        text: window.catalogStatus.available
                            ? qsTr("Categories will be read from the local AppStream catalogue.")
                            : qsTr("No catalogue is available yet.")
                    }
                }
            }

            ScrollView {
                clip: true

                ColumnLayout {
                    width: parent.width
                    spacing: 18
                    anchors.margins: 32

                    PageTitle { text: qsTr("Search") }

                    TextField {
                        id: searchField
                        Layout.fillWidth: true
                        placeholderText: qsTr("Search by name, keyword or package")
                        enabled: window.catalogStatus.available
                        onTextChanged: window.scheduleSearch()
                    }

                    MutedText {
                        Layout.fillWidth: true
                        text: window.catalogStatus.available
                            ? qsTr("Local FTS search is ready for catalogue integration.")
                            : qsTr("Search is disabled until the local catalogue exists.")
                    }

                    Repeater {
                        model: window.searchStatus.results || []

                        delegate: Frame {
                            required property var modelData
                            Layout.fillWidth: true
                            padding: 16

                            RowLayout {
                                anchors.fill: parent
                                spacing: 16

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 4

                                    Label {
                                        Layout.fillWidth: true
                                        text: modelData.name || modelData.id
                                        font.weight: Font.DemiBold
                                    }

                                    MutedText {
                                        Layout.fillWidth: true
                                        text: modelData.summary || qsTr("No summary")
                                    }

                                    MutedText {
                                        Layout.fillWidth: true
                                        text: [modelData.source, modelData.packageAttr]
                                            .filter(value => value)
                                            .join(" · ")
                                    }
                                }

                                Button {
                                    text: StoreActions.busy ? qsTr("Installing…") : qsTr("Install")
                                    enabled: !StoreActions.busy && modelData.source === "nixpkgs"
                                    onClicked: StoreActions.installId = modelData.id
                                }
                            }
                        }
                    }

                    MutedText {
                        visible: window.actionStatus.phase !== undefined
                        Layout.fillWidth: true
                        color: window.actionStatus.available === false
                            ? window.palette.brightText
                            : window.palette.placeholderText
                        text: window.actionStatus.available === false
                            ? (window.actionStatus.error || qsTr("Installation failed"))
                            : (window.actionStatus.phase === "complete"
                                ? qsTr("Installation complete")
                                : qsTr("Planning installation…"))
                    }

                    MutedText {
                        visible: searchField.text.length > 0
                            && (window.searchStatus.results || []).length === 0
                            && window.searchStatus.available !== false
                        Layout.fillWidth: true
                        text: qsTr("No matching applications")
                    }
                }
            }

            ScrollView {
                clip: true

                ColumnLayout {
                    width: parent.width
                    spacing: 18
                    anchors.margins: 32

                    PageTitle { text: qsTr("Sources") }

                    SourceCard {
                        sourceName: qsTr("Nixpkgs")
                        detail: qsTr("Vesper default · follows the locked system revision")
                        enabled: window.sourceStatus.nixpkgs?.enabled ?? true
                        primary: true
                    }

                    SourceCard {
                        sourceName: qsTr("Flathub")
                        detail: qsTr("Optional sandboxed applications · disabled by default")
                        enabled: window.sourceStatus.flathub?.enabled ?? false
                        primary: false
                    }
                }
            }
        }
    }

    Shortcut {
        sequence: StandardKey.Find
        onActivated: {
            window.pageIndex = 2;
            searchField.forceActiveFocus();
        }
    }
}
