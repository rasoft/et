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
    if (!readU64(metadata.value(QStringLiteral("userAreaBytes")), &view.userAreaBytes)) {
        *error = QStringLiteral("文档视图缺少 userAreaBytes");
        return false;
    }
    if (!readU64(metadata.value(QStringLiteral("alignment")), &view.alignment)) {
        *error = QStringLiteral("文档视图缺少 alignment");
        return false;
    }
    if (!root.value(QStringLiteral("partitions")).isArray()
        || !root.value(QStringLiteral("issues")).isArray()) {
        *error = QStringLiteral("文档视图缺少分区或校验列表");
        return false;
    }
    view.issueCount = root.value(QStringLiteral("issues")).toArray().size();
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
