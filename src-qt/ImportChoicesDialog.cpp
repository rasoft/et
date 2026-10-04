#include "ImportChoicesDialog.h"

#include <QAbstractItemView>
#include <QCheckBox>
#include <QDialogButtonBox>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QMessageBox>
#include <QPushButton>
#include <QTableWidget>
#include <QTableWidgetItem>
#include <QVBoxLayout>

namespace {

QString formatBytes(quint64 bytes) {
    const quint64 kib = 1024;
    const quint64 mib = kib * 1024;
    const quint64 gib = mib * 1024;
    const quint64 tib = gib * 1024;
    if (bytes >= tib && bytes % tib == 0) {
        return QStringLiteral("%1 TiB").arg(bytes / tib);
    }
    if (bytes >= gib && bytes % gib == 0) {
        return QStringLiteral("%1 GiB").arg(bytes / gib);
    }
    if (bytes >= mib && bytes % mib == 0) {
        return QStringLiteral("%1 MiB").arg(bytes / mib);
    }
    if (bytes >= kib && bytes % kib == 0) {
        return QStringLiteral("%1 KiB").arg(bytes / kib);
    }
    return QStringLiteral("%1 字节").arg(bytes);
}

QString sizeText(bool hasSize, quint64 bytes) {
    return hasSize ? formatBytes(bytes) : QStringLiteral("剩余空间");
}

void showWarning(QWidget *parent, const QString &text) {
    QMessageBox box(parent);
    box.setIcon(QMessageBox::Warning);
    box.setWindowTitle(QStringLiteral("导入"));
    box.setText(text);
    box.addButton(QStringLiteral("确定"), QMessageBox::AcceptRole);
    box.exec();
}

QTableWidget *makeTable(const QStringList &headers, QWidget *parent) {
    auto *table = new QTableWidget(0, headers.size(), parent);
    table->setHorizontalHeaderLabels(headers);
    table->verticalHeader()->setVisible(false);
    table->setEditTriggers(QAbstractItemView::NoEditTriggers);
    table->setSelectionMode(QAbstractItemView::NoSelection);
    table->setFocusPolicy(Qt::NoFocus);
    table->horizontalHeader()->setStretchLastSection(true);
    table->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
    return table;
}

void fillCell(QTableWidget *table, int row, int column, const QString &text) {
    auto *item = new QTableWidgetItem(text);
    item->setFlags(Qt::ItemIsEnabled);
    table->setItem(row, column, item);
}

} // namespace

ImportChoicesDialog::ImportChoicesDialog(const QString &sourceName, const ImportPreview &preview,
                                         QWidget *parent)
    : QDialog(parent) {
    setWindowTitle(QStringLiteral("导入"));
    resize(720, 560);

    auto *source = new QLabel(QStringLiteral("文件：%1").arg(sourceName), this);
    source->setWordWrap(true);
    source->setTextInteractionFlags(Qt::TextSelectableByMouse);

    m_importTable = new QCheckBox(QStringLiteral("导入分区表"), this);
    m_importTable->setChecked(true);

    const QString tableKind = preview.tableType.isEmpty() ? QStringLiteral("分区表") : preview.tableType;
    QString detail = QStringLiteral("%1，扇区 %2 字节").arg(tableKind).arg(preview.sectorSize);
    detail += preview.hasUserAreaBytes
                  ? QStringLiteral("，容量 %1").arg(formatBytes(preview.userAreaBytes))
                  : QStringLiteral("，容量在下载时确定");
    auto *tableDetail = new QLabel(detail, this);
    tableDetail->setWordWrap(true);

    auto *partitions = makeTable({QStringLiteral("分区"), QStringLiteral("起始"), QStringLiteral("大小")}, this);
    partitions->setRowCount(preview.partitions.size());
    for (int row = 0; row < preview.partitions.size(); ++row) {
        const ImportPartitionChoice &part = preview.partitions.at(row);
        fillCell(partitions, row, 0, part.name);
        fillCell(partitions, row, 1, formatBytes(part.startBytes));
        fillCell(partitions, row, 2, sizeText(part.hasSizeBytes, part.sizeBytes));
    }
    partitions->setMinimumHeight(140);

    auto *imageTitle = new QLabel(QStringLiteral("镜像文件"), this);
    m_images = makeTable({QStringLiteral("文件"), QStringLiteral("分区"), QStringLiteral("大小")}, this);
    m_images->setRowCount(preview.images.size());
    m_images->setMinimumHeight(160);
    for (int row = 0; row < preview.images.size(); ++row) {
        const ImportImageChoice &image = preview.images.at(row);
        auto *file = new QTableWidgetItem(image.fileName);
        file->setData(Qt::UserRole, image.index);
        if (image.available) {
            file->setFlags(Qt::ItemIsEnabled | Qt::ItemIsUserCheckable);
            file->setCheckState(Qt::Checked);
        } else {
            file->setFlags(Qt::NoItemFlags);
            file->setToolTip(image.message);
        }
        m_images->setItem(row, 0, file);
        fillCell(m_images, row, 1, image.partitionName);
        if (image.available && image.hasBytes) {
            fillCell(m_images, row, 2, formatBytes(image.bytes));
        } else {
            const QString note = image.message.isEmpty() ? QStringLiteral("不可导入") : image.message;
            fillCell(m_images, row, 2, note);
        }
    }

    auto *emptyImages = new QLabel(QStringLiteral("没有镜像文件。"), this);
    emptyImages->setVisible(preview.images.isEmpty());
    m_images->setVisible(!preview.images.isEmpty());

    auto *hint = new QLabel(
        QStringLiteral("可以只导入分区表，或只导入选中的镜像。只导入镜像时，按分区名称写入当前包，不改变分区的位置和大小。"),
        this);
    hint->setWordWrap(true);

    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    buttons->button(QDialogButtonBox::Ok)->setText(QStringLiteral("导入"));
    buttons->button(QDialogButtonBox::Cancel)->setText(QStringLiteral("取消"));

    auto *layout = new QVBoxLayout(this);
    layout->addWidget(source);
    layout->addWidget(m_importTable);
    layout->addWidget(tableDetail);
    layout->addWidget(partitions, 1);
    layout->addWidget(imageTitle);
    layout->addWidget(emptyImages);
    layout->addWidget(m_images, 1);
    layout->addWidget(hint);
    layout->addWidget(buttons);

    connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
}

QString ImportChoicesDialog::selectionJson() const {
    return m_selectionJson;
}

void ImportChoicesDialog::accept() {
    QJsonArray images;
    bool anyImage = false;
    for (int row = 0; row < m_images->rowCount(); ++row) {
        const QTableWidgetItem *item = m_images->item(row, 0);
        if (item == nullptr || item->checkState() != Qt::Checked) {
            continue;
        }
        anyImage = true;
        images.append(item->data(Qt::UserRole).toInt());
    }
    if (!m_importTable->isChecked() && !anyImage) {
        showWarning(this, QStringLiteral("请选择分区表，或至少一个镜像文件"));
        return;
    }
    QJsonObject selection;
    selection.insert(QStringLiteral("importTable"), m_importTable->isChecked());
    selection.insert(QStringLiteral("images"), images);
    m_selectionJson = QString::fromUtf8(QJsonDocument(selection).toJson(QJsonDocument::Compact));
    QDialog::accept();
}
