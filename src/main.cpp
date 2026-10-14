#include <QFileInfo>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QProcess>
#include <QQmlPropertyMap>
#include <QUrl>

namespace {
QString runCore(const QStringList &arguments) {
    const QString core = qEnvironmentVariable("VESPER_STORE_CORE");
    if (core.isEmpty() || !QFileInfo::exists(core))
        return QStringLiteral("{}");

    QProcess process;
    process.start(core, arguments);
    if (!process.waitForFinished(3000) || process.exitStatus() != QProcess::NormalExit || process.exitCode() != 0)
        return QStringLiteral("{}");

    return QString::fromUtf8(process.readAllStandardOutput()).trimmed();
}
}

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("Vesper Store"));
    app.setApplicationDisplayName(QStringLiteral("Vesper Store"));
    app.setOrganizationName(QStringLiteral("Vesper"));
    app.setDesktopFileName(QStringLiteral("io.vesper.Store"));

    QString initialSearch;
    const QStringList arguments = app.arguments();
    for (qsizetype index = 1; index + 1 < arguments.size(); ++index) {
        if (arguments.at(index) == QStringLiteral("--search")) {
            initialSearch = arguments.at(++index).trimmed();
            break;
        }
    }

    QQmlApplicationEngine engine;
    const QString core = qEnvironmentVariable("VESPER_STORE_CORE");
    QQmlPropertyMap searchState;
    searchState.insert(
        QStringLiteral("json"),
        QStringLiteral("{\"available\":false,\"query\":\"\",\"results\":[]}"));
    searchState.insert(QStringLiteral("busy"), false);
    searchState.insert(QStringLiteral("query"), QString());
    QQmlPropertyMap actionState;
    actionState.insert(QStringLiteral("json"), QStringLiteral("{}"));
    actionState.insert(QStringLiteral("busy"), false);
    actionState.insert(QStringLiteral("installId"), QString());

    QProcess searchProcess;
    QObject::connect(
        &searchState,
        &QQmlPropertyMap::valueChanged,
        &app,
        [&](const QString &key, const QVariant &value) {
            if (key != QStringLiteral("query"))
                return;

            const QString query = value.toString().trimmed();
            if (searchProcess.state() != QProcess::NotRunning) {
                searchProcess.kill();
                searchProcess.waitForFinished(1000);
            }
            if (query.isEmpty()) {
                searchState.insert(
                    QStringLiteral("json"),
                    QStringLiteral("{\"available\":true,\"query\":\"\",\"results\":[]}"));
                searchState.insert(QStringLiteral("busy"), false);
                return;
            }
            if (core.isEmpty() || !QFileInfo::exists(core)) {
                searchState.insert(
                    QStringLiteral("json"),
                    QStringLiteral("{\"available\":false,\"query\":\"\",\"results\":[],\"error\":\"Store backend unavailable\"}"));
                searchState.insert(QStringLiteral("busy"), false);
                return;
            }

            searchState.insert(QStringLiteral("busy"), true);
            searchProcess.start(core, {QStringLiteral("search"), query});
        });
    QObject::connect(
        &searchProcess,
        &QProcess::finished,
        &app,
        [&](int exitCode, QProcess::ExitStatus exitStatus) {
            const QString result = QString::fromUtf8(searchProcess.readAllStandardOutput()).trimmed();
            if (exitStatus == QProcess::NormalExit && exitCode == 0 && !result.isEmpty()) {
                searchState.insert(QStringLiteral("json"), result);
            } else {
                searchState.insert(
                    QStringLiteral("json"),
                    QStringLiteral("{\"available\":false,\"query\":\"\",\"results\":[],\"error\":\"Store search failed\"}"));
            }
            searchState.insert(QStringLiteral("busy"), false);
        });

    QProcess actionProcess;
    QObject::connect(
        &actionState,
        &QQmlPropertyMap::valueChanged,
        &app,
        [&](const QString &key, const QVariant &value) {
            if (key != QStringLiteral("installId") || actionProcess.state() != QProcess::NotRunning)
                return;
            const QString appId = value.toString().trimmed();
            if (appId.isEmpty() || core.isEmpty() || !QFileInfo::exists(core))
                return;
            actionState.insert(QStringLiteral("busy"), true);
            actionState.insert(
                QStringLiteral("json"),
                QStringLiteral("{\"available\":true,\"phase\":\"planning\"}"));
            actionProcess.start(core, {QStringLiteral("nix-install"), appId});
        });
    QObject::connect(
        &actionProcess,
        &QProcess::finished,
        &app,
        [&](int exitCode, QProcess::ExitStatus exitStatus) {
            const QString result = QString::fromUtf8(actionProcess.readAllStandardOutput()).trimmed();
            actionState.insert(
                QStringLiteral("json"),
                exitStatus == QProcess::NormalExit && exitCode == 0 && !result.isEmpty()
                    ? result
                    : QStringLiteral("{\"available\":false,\"phase\":\"failed\",\"error\":\"Store transaction failed\"}"));
            actionState.insert(QStringLiteral("busy"), false);
            actionState.insert(QStringLiteral("installId"), QString());
        });

    engine.rootContext()->setContextProperty(QStringLiteral("StoreSearch"), &searchState);
    engine.rootContext()->setContextProperty(QStringLiteral("StoreActions"), &actionState);
    engine.rootContext()->setContextProperty(QStringLiteral("StoreInitialSearch"), initialSearch);
    engine.rootContext()->setContextProperty(
        QStringLiteral("StoreCatalogStatusJson"),
        runCore({QStringLiteral("catalog-status")})
    );
    engine.rootContext()->setContextProperty(
        QStringLiteral("StoreSourcesJson"),
        runCore({QStringLiteral("sources")})
    );

    const QString qmlPath = qEnvironmentVariable("VESPER_STORE_QML");
    if (qmlPath.isEmpty() || !QFileInfo::exists(qmlPath))
        return 2;

    engine.load(QUrl::fromLocalFile(qmlPath));
    if (engine.rootObjects().isEmpty())
        return 3;

    return app.exec();
}
