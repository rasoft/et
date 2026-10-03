#include "MainWindow.h"
#include "et_abi.h"

#include <QApplication>
#include <QIcon>

int main(int argc, char *argv[]) {
    QApplication::setAttribute(Qt::AA_EnableHighDpiScaling);
    QApplication::setAttribute(Qt::AA_UseHighDpiPixmaps);

    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("et"));

    QIcon icon;
    const int iconSizes[] = {16, 24, 32, 48, 64, 128, 256};
    for (int iconSize : iconSizes) {
        icon.addFile(QStringLiteral(":/icons/%1.png").arg(iconSize));
    }
    QApplication::setWindowIcon(icon);

    if (et_abi_version() < 1 || et_version() == nullptr) {
        return 1;
    }

    MainWindow window;
    window.show();
    return app.exec();
}
