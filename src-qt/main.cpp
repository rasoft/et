#include "MainWindow.h"
#include "et_abi.h"

#include <QApplication>

int main(int argc, char *argv[]) {
    QApplication::setAttribute(Qt::AA_EnableHighDpiScaling);
    QApplication::setAttribute(Qt::AA_UseHighDpiPixmaps);

    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("et"));

    if (et_abi_version() < 1 || et_version() == nullptr) {
        return 1;
    }

    MainWindow window;
    window.show();
    return app.exec();
}
