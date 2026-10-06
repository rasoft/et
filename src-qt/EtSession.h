#pragma once

#include <QString>
#include <QStringList>
#include <QVector>

struct PartitionView {
    QString id;
    QString name;
    QString type;
    quint64 attributes = 0;
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
    QString archive;
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

struct ImportPartitionChoice {
    QString name;
    quint64 startBytes = 0;
    bool hasSizeBytes = false;
    quint64 sizeBytes = 0;
};

struct ImportImageChoice {
    quint32 index = 0;
    QString partitionName;
    QString fileName;
    bool available = false;
    bool hasBytes = false;
    quint64 bytes = 0;
    QString message;
};

struct ImportPreview {
    quint32 sectorSize = 512;
    bool hasUserAreaBytes = false;
    quint64 userAreaBytes = 0;
    QString tableType;
    QVector<ImportPartitionChoice> partitions;
    QVector<ImportImageChoice> images;

    static bool parse(const QString &json, ImportPreview *out, QString *error);
};

namespace EtSession {

bool createPackage(bool discardUnsaved, QString *viewJson, QString *error);

bool openPackage(const QString &archiveFile, bool discardUnsaved, QString *viewJson, QString *error);

bool savePackage(const QString &archiveFile, QString *viewJson, QString *error);

void closePackage();

bool previewImport(const QString &sourceFile, QString *previewJson, QString *error);

bool importPackage(const QString &sourceFile, const QString &selectionJson, QString *viewJson,
                   QString *error);

bool removePartitions(const QStringList &ids, QString *viewJson, QString *error);

}
