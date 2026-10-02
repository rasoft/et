#include "et_abi.h"

#include <QApplication>
#include <QMainWindow>

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("et"));

    if (et_abi_version() < 1 || et_version() == nullptr) {
        return 1;
    }

    QMainWindow window;
    window.setWindowTitle(QStringLiteral("et"));
    window.resize(1024, 700);
    window.show();
    return app.exec();
}
