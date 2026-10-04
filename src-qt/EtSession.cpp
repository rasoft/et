#include "EtSession.h"

#include "et_abi.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>

#include <cmath>

namespace {

QString takeString(char *text) {
    if (text == nullptr) {
        return {};
    }
    const QString copy = QString::fromUtf8(text);
    et_string_free(text);
    return copy;
}

bool readString(const QJsonObject &object, const QString &key, QString *out, QString *error) {
    const QJsonValue value = object.value(key);
    if (!value.isString()) {
        *error = QStringLiteral("文档视图缺少 %1").arg(key);
        return false;
    }
    *out = value.toString();
    return true;
}

bool readBool(const QJsonObject &object, const QString &key, bool *out, QString *error) {
    const QJsonValue value = object.value(key);
    if (!value.isBool()) {
        *error = QStringLiteral("文档视图缺少 %1").arg(key);
        return false;
    }
    *out = value.toBool();
    return true;
}

bool readU64(const QJsonValue &value, quint64 *out) {
    if (!value.isDouble()) {
        return false;
    }
    const double number = value.toDouble();
    if (!std::isfinite(number) || number < 0.0) {
        return false;
    }
    if (number != std::floor(number)) {
        return false;
    }
    *out = static_cast<quint64>(number);
    return true;
}

bool readOptionalString(const QJsonObject &object, const QString &key, bool *present, QString *out,
                        QString *error) {
    const QJsonValue value = object.value(key);
    if (value.isNull()) {
        *present = false;
        out->clear();
        return true;
    }
    if (!value.isString()) {
        *error = QStringLiteral("文档视图缺少 %1").arg(key);
        return false;
    }
    *present = true;
    *out = value.toString();
    return true;
}

bool readOptionalU64(const QJsonObject &object, const QString &key, bool *present, quint64 *out,
                     QString *error) {
    const QJsonValue value = object.value(key);
    if (value.isNull()) {
        *present = false;
        *out = 0;
        return true;
    }
    if (!readU64(value, out)) {
        *error = QStringLiteral("文档视图缺少 %1").arg(key);
        return false;
    }
    *present = true;
    return true;
}

} // namespace

bool DocumentView::parse(const QString &json, DocumentView *out, QString *error) {
    if (out == nullptr || error == nullptr) {
        return false;
    }
    QJsonParseError parseError;
    const QJsonDocument document = QJsonDocument::fromJson(json.toUtf8(), &parseError);
    if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
        *error = QStringLiteral("命令返回的文档无法解析");
        return false;
    }
    const QJsonObject root = document.object();
    DocumentView view;
    if (!readString(root, QStringLiteral("root"), &view.root, error)) {
        return false;
    }
    if (!readBool(root, QStringLiteral("dirty"), &view.dirty, error)
        || !readBool(root, QStringLiteral("canUndo"), &view.canUndo, error)
        || !readBool(root, QStringLiteral("canRedo"), &view.canRedo, error)) {
        return false;
    }
    const QJsonValue metadataValue = root.value(QStringLiteral("metadata"));
    if (!metadataValue.isObject()) {
        *error = QStringLiteral("文档视图缺少 metadata");
        return false;
    }
    const QJsonObject metadata = metadataValue.toObject();
    if (!readString(metadata, QStringLiteral("name"), &view.name, error)
        || !readString(metadata, QStringLiteral("description"), &view.description, error)) {
        return false;
    }
    quint64 sector = 0;
    if (!readU64(metadata.value(QStringLiteral("sectorSize")), &sector) || sector > 0xffffffffu) {
        *error = QStringLiteral("文档视图缺少 sectorSize");
        return false;
    }
    view.sectorSize = static_cast<quint32>(sector);
    if (!readOptionalU64(metadata, QStringLiteral("userAreaBytes"), &view.hasUserAreaBytes,
                         &view.userAreaBytes, error)) {
        return false;
    }
    if (!readU64(metadata.value(QStringLiteral("alignment")), &view.alignment)) {
        *error = QStringLiteral("文档视图缺少 alignment");
        return false;
    }
    const QJsonValue partitionsValue = root.value(QStringLiteral("partitions"));
    const QJsonValue issuesValue = root.value(QStringLiteral("issues"));
    if (!partitionsValue.isArray() || !issuesValue.isArray()) {
        *error = QStringLiteral("文档视图缺少分区或校验列表");
        return false;
    }
    const QJsonArray partitions = partitionsValue.toArray();
    view.partitions.reserve(partitions.size());
    for (const QJsonValue &entry : partitions) {
        if (!entry.isObject()) {
            *error = QStringLiteral("文档视图的分区不是对象");
            return false;
        }
        const QJsonObject object = entry.toObject();
        PartitionView partition;
        if (!readString(object, QStringLiteral("id"), &partition.id, error)
            || !readString(object, QStringLiteral("name"), &partition.name, error)
            || !readString(object, QStringLiteral("type"), &partition.type, error)) {
            return false;
        }
        if (!readU64(object.value(QStringLiteral("startBytes")), &partition.startBytes)) {
            *error = QStringLiteral("文档视图缺少 startBytes");
            return false;
        }
        if (!readBool(object, QStringLiteral("startFixed"), &partition.startFixed, error)) {
            return false;
        }
        if (!readOptionalU64(object, QStringLiteral("sizeBytes"), &partition.hasSizeBytes,
                             &partition.sizeBytes, error)) {
            return false;
        }
        if (!readOptionalString(object, QStringLiteral("image"), &partition.hasImage, &partition.image,
                                error)
            || !readOptionalU64(object, QStringLiteral("imageBytes"), &partition.hasImageBytes,
                                &partition.imageBytes, error)) {
            return false;
        }
        view.partitions.append(partition);
    }
    const QJsonArray issues = issuesValue.toArray();
    view.issues.reserve(issues.size());
    for (const QJsonValue &entry : issues) {
        if (!entry.isObject()) {
            *error = QStringLiteral("文档视图的校验项不是对象");
            return false;
        }
        const QJsonObject object = entry.toObject();
        IssueView issue;
        if (!readString(object, QStringLiteral("severity"), &issue.severity, error)
            || !readString(object, QStringLiteral("message"), &issue.message, error)
            || !readOptionalString(object, QStringLiteral("partitionId"), &issue.hasPartitionId,
                                   &issue.partitionId, error)) {
            return false;
        }
        view.issues.append(issue);
    }
    view.issueCount = view.issues.size();
    *out = view;
    return true;
}

bool EtSession::createPackage(const QString &directory, const QString &name, quint64 userAreaBytes,
                              quint32 sectorSize, bool discardUnsaved, QString *viewJson,
                              QString *error) {
    if (viewJson == nullptr || error == nullptr) {
        return false;
    }
    // 容量和扇区走整数参数。QJson 的数字是 double，大于 2^53 的字节数会失真。
    const QByteArray directoryBytes = directory.toUtf8();
    const QByteArray nameBytes = name.toUtf8();
    char *view = nullptr;
    char *message = nullptr;
    const int32_t rc = et_create_package(directoryBytes.constData(), nameBytes.constData(),
                                         userAreaBytes, sectorSize, discardUnsaved ? 1 : 0, &view,
                                         &message);
    const QString viewText = takeString(view);
    const QString errorText = takeString(message);
    if (rc != 0) {
        *error = errorText.isEmpty() ? QStringLiteral("新建失败") : errorText;
        return false;
    }
    if (viewText.isEmpty()) {
        *error = QStringLiteral("新建没有返回文档");
        return false;
    }
    *viewJson = viewText;
    return true;
}

bool EtSession::openPackage(const QString &directory, bool discardUnsaved, QString *viewJson,
                            QString *error) {
    if (viewJson == nullptr || error == nullptr) {
        return false;
    }
    const QByteArray directoryBytes = directory.toUtf8();
    char *view = nullptr;
    char *message = nullptr;
    const int32_t rc =
        et_open_package(directoryBytes.constData(), discardUnsaved ? 1 : 0, &view, &message);
    const QString viewText = takeString(view);
    const QString errorText = takeString(message);
    if (rc != 0) {
        *error = errorText.isEmpty() ? QStringLiteral("打开失败") : errorText;
        return false;
    }
    if (viewText.isEmpty()) {
        *error = QStringLiteral("打开没有返回文档");
        return false;
    }
    *viewJson = viewText;
    return true;
}

bool EtSession::importPackage(const QString &sourceFile, QString *viewJson, QString *error) {
    if (viewJson == nullptr || error == nullptr) {
        return false;
    }
    const QByteArray sourceBytes = sourceFile.toUtf8();
    char *view = nullptr;
    char *message = nullptr;
    const int32_t rc = et_import_package(sourceBytes.constData(), &view, &message);
    const QString viewText = takeString(view);
    const QString errorText = takeString(message);
    if (rc != 0) {
        *error = errorText.isEmpty() ? QStringLiteral("导入失败") : errorText;
        return false;
    }
    if (viewText.isEmpty()) {
        *error = QStringLiteral("导入没有返回文档");
        return false;
    }
    *viewJson = viewText;
    return true;
}
