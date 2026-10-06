#include "ImportChoicesDialog.h"

#include <QAbstractItemView>
#include <QCheckBox>
#include <QDialogButtonBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QMessageBox>
#include <QPushButton>
#include <QResizeEvent>
#include <QScrollBar>
#include <QShowEvent>
#include <QStyle>
#include <QStyleOptionButton>
#include <QTabWidget>
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
    table->setWordWrap(false);
    table->setTextElideMode(Qt::ElideRight);
    table->horizontalHeader()->setStretchLastSection(true);
    table->horizontalHeader()->setHighlightSections(false);
    return table;
}

void fillCell(QTableWidget *table, int row, int column, const QString &text,
              int alignment = Qt::AlignLeft | Qt::AlignVCenter) {
    auto *item = new QTableWidgetItem(text);
    item->setFlags(Qt::ItemIsEnabled);
    item->setTextAlignment(alignment);
    table->setItem(row, column, item);
}

const PartitionView *findCurrentPartition(const QVector<PartitionView> &partitions, const QString &name) {
    for (const PartitionView &part : partitions) {
        if (part.name == name) {
            return &part;
        }
    }
    return nullptr;
}

class HeaderCheckBox : public QCheckBox {
public:
    using QCheckBox::QCheckBox;

protected:
    void nextCheckState() override {
        setCheckState(checkState() == Qt::Checked ? Qt::Unchecked : Qt::Checked);
    }
};

class ImageCheckHeader : public QHeaderView {
public:
    explicit ImageCheckHeader(QWidget *parent)
        : QHeaderView(Qt::Horizontal, parent) {
        setSectionsClickable(false);
        m_check = new HeaderCheckBox(viewport());
        m_check->setTristate(true);
        m_check->setFocusPolicy(Qt::NoFocus);
        m_check->setText(QString());
    }

    QCheckBox *checkBox() const { return m_check; }

    void updateCheckGeometry() {
        if (m_check == nullptr) {
            return;
        }
        if (count() <= 0 || sectionSize(0) <= 0) {
            m_check->hide();
            return;
        }
        m_check->show();
        const int sectionX = sectionViewportPosition(0);
        const int sectionW = sectionSize(0);
        const QSize hint = m_check->sizeHint();
        QStyleOptionButton option;
        option.initFrom(m_check);
        option.rect = QRect(QPoint(0, 0), hint);
        const QRect indicator = m_check->style()->subElementRect(QStyle::SE_CheckBoxIndicator, &option, m_check);
        const int glyphWidth = indicator.isValid() ? indicator.width() : hint.width();
        const int glyphX = indicator.isValid() ? indicator.x() : 0;
        const int x = sectionX + (sectionW - glyphWidth) / 2 - glyphX;
        const int y = (height() - hint.height()) / 2;
        m_check->setGeometry(x, y, hint.width(), hint.height());
        m_check->raise();
    }

protected:
    void resizeEvent(QResizeEvent *event) override {
        QHeaderView::resizeEvent(event);
        updateCheckGeometry();
    }

    void showEvent(QShowEvent *event) override {
        QHeaderView::showEvent(event);
        updateCheckGeometry();
    }

private:
    HeaderCheckBox *m_check = nullptr;
};

QWidget *checkCell(QCheckBox *box, QWidget *parent) {
    auto *holder = new QWidget(parent);
    holder->setObjectName(QStringLiteral("importCheckCell"));
    holder->setStyleSheet(QStringLiteral("#importCheckCell { background: transparent; }"));
    auto *layout = new QHBoxLayout(holder);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setAlignment(Qt::AlignCenter);
    layout->addWidget(box);
    return holder;
}

} // namespace

ImportChoicesDialog::ImportChoicesDialog(const QString &sourceName, const ImportPreview &preview,
                                         const QVector<PartitionView> &currentPartitions, QWidget *parent)
    : QDialog(parent) {
    setWindowTitle(QStringLiteral("导入"));
    resize(760, 520);

    auto *source = new QLabel(QStringLiteral("文件：%1").arg(sourceName), this);
    source->setWordWrap(true);
    source->setTextInteractionFlags(Qt::TextSelectableByMouse);

    auto *tablePage = new QWidget(this);
    m_importTable = new QCheckBox(QStringLiteral("覆盖当前分区表"), tablePage);
    m_importTable->setChecked(true);

    const QString tableKind = preview.tableType.isEmpty() ? QStringLiteral("分区表") : preview.tableType;
    QString detail = QStringLiteral("%1，扇区 %2 字节").arg(tableKind).arg(preview.sectorSize);
    detail += preview.hasUserAreaBytes
                  ? QStringLiteral("，容量 %1").arg(formatBytes(preview.userAreaBytes))
                  : QStringLiteral("，容量在下载时确定");
    auto *tableDetail = new QLabel(detail, tablePage);
    tableDetail->setWordWrap(true);

    auto *partitions = makeTable({QStringLiteral("分区"), QStringLiteral("起始"), QStringLiteral("大小")}, tablePage);
    partitions->setRowCount(preview.partitions.size());
    const int alignRight = Qt::AlignRight | Qt::AlignVCenter;
    partitions->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
    partitions->horizontalHeader()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
    partitions->horizontalHeader()->setSectionResizeMode(2, QHeaderView::ResizeToContents);
    for (int row = 0; row < preview.partitions.size(); ++row) {
        const ImportPartitionChoice &part = preview.partitions.at(row);
        fillCell(partitions, row, 0, part.name);
        fillCell(partitions, row, 1, formatBytes(part.startBytes), alignRight);
        fillCell(partitions, row, 2, sizeText(part.hasSizeBytes, part.sizeBytes), alignRight);
    }

    auto *tableLayout = new QVBoxLayout(tablePage);
    tableLayout->setContentsMargins(0, 8, 0, 0);
    tableLayout->addWidget(m_importTable);
    tableLayout->addWidget(tableDetail);
    tableLayout->addWidget(partitions, 1);

    auto *imagePage = new QWidget(this);
    auto *images = makeTable(
        {QString(), QStringLiteral("文件"), QStringLiteral("分区"), QStringLiteral("大小"), QStringLiteral("说明")},
        imagePage);
    auto *imageHeader = new ImageCheckHeader(images);
    images->setHorizontalHeader(imageHeader);
    m_imageHeaderCheck = imageHeader->checkBox();
    images->setRowCount(preview.images.size());
    imageHeader->setHighlightSections(false);
    imageHeader->setStretchLastSection(false);
    imageHeader->setMinimumSectionSize(28);
    imageHeader->setSectionResizeMode(0, QHeaderView::Fixed);
    imageHeader->setSectionResizeMode(1, QHeaderView::Stretch);
    imageHeader->setSectionResizeMode(2, QHeaderView::ResizeToContents);
    imageHeader->setSectionResizeMode(3, QHeaderView::ResizeToContents);
    imageHeader->setSectionResizeMode(4, QHeaderView::Stretch);
    images->setColumnWidth(0, 44);
    imageHeader->updateCheckGeometry();
    connect(m_imageHeaderCheck, &QCheckBox::clicked, this, [this](bool) { applyImageHeaderCheck(); });
    connect(imageHeader, &QHeaderView::sectionResized, this,
            [imageHeader](int, int, int) { imageHeader->updateCheckGeometry(); });
    connect(images->horizontalScrollBar(), &QScrollBar::valueChanged, this,
            [imageHeader](int) { imageHeader->updateCheckGeometry(); });
    images->verticalHeader()->setDefaultSectionSize(28);

    m_imageRows.reserve(preview.images.size());
    for (int row = 0; row < preview.images.size(); ++row) {
        const ImportImageChoice &image = preview.images.at(row);
        ImageChoiceRow choice;
        choice.index = image.index;
        choice.fileMissing = !image.available;
        choice.fileMessage = image.message.isEmpty() ? QStringLiteral("不可导入") : image.message;
        choice.preferred = image.available;
        if (image.available) {
            const PartitionView *current = findCurrentPartition(currentPartitions, image.partitionName);
            if (current == nullptr) {
                choice.partitionMissing = true;
            } else if (image.hasBytes && current->hasSizeBytes && image.bytes > current->sizeBytes) {
                choice.tooSmallMessage =
                    QStringLiteral("分区大小不够大（当前 %1）").arg(formatBytes(current->sizeBytes));
            }
        }

        auto *check = new QCheckBox(images);
        check->setFocusPolicy(Qt::NoFocus);
        choice.check = check;
        images->setCellWidget(row, 0, checkCell(check, images));
        fillCell(images, row, 1, image.fileName);
        fillCell(images, row, 2, image.partitionName);
        fillCell(images, row, 3, image.hasBytes ? formatBytes(image.bytes) : QStringLiteral("—"), alignRight);
        auto *note = new QTableWidgetItem;
        note->setFlags(Qt::ItemIsEnabled);
        images->setItem(row, 4, note);
        choice.note = note;

        const int rowIndex = m_imageRows.size();
        m_imageRows.append(choice);
        connect(check, &QCheckBox::toggled, this, [this, rowIndex](bool checked) {
            if (m_refreshing || rowIndex < 0 || rowIndex >= m_imageRows.size()) {
                return;
            }
            if (!m_imageRows.at(rowIndex).check->isEnabled()) {
                return;
            }
            m_imageRows[rowIndex].preferred = checked;
            updateImageHeaderCheck();
        });
    }

    auto *emptyImages = new QLabel(QStringLiteral("没有镜像文件。"), imagePage);
    emptyImages->setVisible(preview.images.isEmpty());
    images->setVisible(!preview.images.isEmpty());

    auto *hint = new QLabel(
        QStringLiteral("未覆盖分区表时，镜像按分区名称写入当前包，不改变分区的位置和大小。当前包没有同名分区，或分区放不下该镜像时，不能勾选。"),
        imagePage);
    hint->setWordWrap(true);
    hint->setVisible(!preview.images.isEmpty());

    auto *imageLayout = new QVBoxLayout(imagePage);
    imageLayout->setContentsMargins(0, 8, 0, 0);
    imageLayout->addWidget(emptyImages);
    imageLayout->addWidget(images, 1);
    imageLayout->addWidget(hint);

    auto *tabs = new QTabWidget(this);
    tabs->addTab(tablePage, QStringLiteral("分区表"));
    tabs->addTab(imagePage, QStringLiteral("镜像文件"));

    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    buttons->button(QDialogButtonBox::Ok)->setText(QStringLiteral("导入"));
    buttons->button(QDialogButtonBox::Cancel)->setText(QStringLiteral("取消"));

    auto *layout = new QVBoxLayout(this);
    layout->addWidget(source);
    layout->addWidget(tabs, 1);
    layout->addWidget(buttons);

    connect(m_importTable, &QCheckBox::toggled, this, [this](bool) { refreshImageChoices(); });
    connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
    refreshImageChoices();
}

QString ImportChoicesDialog::selectionJson() const {
    return m_selectionJson;
}

QString ImportChoicesDialog::blockReason(const ImageChoiceRow &row) const {
    if (row.fileMissing) {
        return row.fileMessage;
    }
    if (m_importTable->isChecked()) {
        return QString();
    }
    if (row.partitionMissing) {
        return QStringLiteral("分区不存在");
    }
    if (!row.tooSmallMessage.isEmpty()) {
        return row.tooSmallMessage;
    }
    return QString();
}

void ImportChoicesDialog::refreshImageChoices() {
    m_refreshing = true;
    for (ImageChoiceRow &row : m_imageRows) {
        const QString reason = blockReason(row);
        const bool enabled = reason.isEmpty();
        row.check->blockSignals(true);
        row.check->setEnabled(enabled);
        row.check->setChecked(enabled && row.preferred);
        row.check->blockSignals(false);
        row.check->setToolTip(reason);
        row.note->setText(reason);
        row.note->setToolTip(reason);
    }
    updateImageHeaderCheck();
    m_refreshing = false;
}

void ImportChoicesDialog::updateImageHeaderCheck() {
    if (m_imageHeaderCheck == nullptr) {
        return;
    }
    int enabled = 0;
    int checked = 0;
    for (const ImageChoiceRow &row : m_imageRows) {
        if (row.check == nullptr || !row.check->isEnabled()) {
            continue;
        }
        ++enabled;
        if (row.check->isChecked()) {
            ++checked;
        }
    }
    Qt::CheckState state = Qt::Unchecked;
    if (enabled > 0 && checked == enabled) {
        state = Qt::Checked;
    } else if (checked > 0) {
        state = Qt::PartiallyChecked;
    }
    m_imageHeaderCheck->blockSignals(true);
    m_imageHeaderCheck->setEnabled(enabled > 0);
    if (m_imageHeaderCheck->checkState() != state) {
        m_imageHeaderCheck->setCheckState(state);
    }
    m_imageHeaderCheck->blockSignals(false);
}

void ImportChoicesDialog::applyImageHeaderCheck() {
    if (m_imageHeaderCheck == nullptr || !m_imageHeaderCheck->isEnabled()) {
        return;
    }
    const bool check = m_imageHeaderCheck->checkState() == Qt::Checked;
    m_refreshing = true;
    for (ImageChoiceRow &row : m_imageRows) {
        if (row.check == nullptr || !row.check->isEnabled()) {
            continue;
        }
        row.preferred = check;
        row.check->setChecked(check);
    }
    m_refreshing = false;
    updateImageHeaderCheck();
}

void ImportChoicesDialog::accept() {
    QJsonArray images;
    bool anyImage = false;
    for (const ImageChoiceRow &row : m_imageRows) {
        if (row.check == nullptr || !row.check->isEnabled() || !row.check->isChecked()) {
            continue;
        }
        anyImage = true;
        images.append(static_cast<int>(row.index));
    }
    if (!m_importTable->isChecked() && !anyImage) {
        showWarning(this, QStringLiteral("请勾选覆盖分区表，或至少一个镜像文件"));
        return;
    }
    QJsonObject selection;
    selection.insert(QStringLiteral("importTable"), m_importTable->isChecked());
    selection.insert(QStringLiteral("images"), images);
    m_selectionJson = QString::fromUtf8(QJsonDocument(selection).toJson(QJsonDocument::Compact));
    QDialog::accept();
}
