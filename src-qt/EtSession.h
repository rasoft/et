#pragma once

#include <QString>

struct DocumentView {
    QString root;
    QString name;
    QString description;
    quint64 userAreaBytes = 0;
    quint32 sectorSize = 0;
    quint64 alignment = 0;
    bool dirty = false;
    bool canUndo = false;
    bool canRedo = false;
    int issueCount = 0;

    static bool parse(const QString &json, DocumentView *out, QString *error);
};

namespace EtSession {

bool createPackage(const QString &directory, const QString &name, quint64 userAreaBytes,
                   quint32 sectorSize, bool discardUnsaved, QString *viewJson, QString *error);

}
