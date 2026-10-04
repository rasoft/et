#pragma once

#include <QString>
#include <QVector>

struct PartitionView {
    QString id;
    QString name;
    QString type;
    quint64 startBytes = 0;
    bool startFixed = false;
    bool hasSizeBytes = false;
    quint64 sizeBytes = 0;
    bool hasImage = false;
    QString image;
    bool hasImageBytes = false;
    quint64 imageBytes = 0;
};

struct IssueView {
    QString severity;
    bool hasPartitionId = false;
    QString partitionId;
    QString message;
};

struct DocumentView {
    QString root;
    QString name;
    QString description;
    bool hasUserAreaBytes = false;
    quint64 userAreaBytes = 0;
    quint32 sectorSize = 0;
    quint64 alignment = 0;
    bool dirty = false;
    bool canUndo = false;
    bool canRedo = false;
    QVector<PartitionView> partitions;
    QVector<IssueView> issues;
    int issueCount = 0;

    static bool parse(const QString &json, DocumentView *out, QString *error);
};

namespace EtSession {

bool createPackage(const QString &directory, const QString &name, quint64 userAreaBytes,
                   quint32 sectorSize, bool discardUnsaved, QString *viewJson, QString *error);

bool openPackage(const QString &directory, bool discardUnsaved, QString *viewJson, QString *error);

bool importPackage(const QString &sourceFile, QString *viewJson, QString *error);

}
