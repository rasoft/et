#include "PartitionEditorWidget.h"

#include "EtSession.h"

#include <QCheckBox>
#include <QFormLayout>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QScrollArea>
#include <QStackedWidget>
#include <QTabWidget>
#include <QVBoxLayout>

namespace {

constexpr quint64 kGptRequired = 1ull << 0;
constexpr quint64 kGptNoBlockIo = 1ull << 1;
constexpr quint64 kGptLegacyBios = 1ull << 2;

QString typeGuidText(const QString &type) {
    const QString key = type.trimmed().toLower();
    if (key == QLatin1String("linux-filesystem")) {
        return QStringLiteral("0FC63DAF-8483-4772-8E79-3D69D8477DE4");
    }
    if (key == QLatin1String("efi-system")) {
        return QStringLiteral("C12A7328-F81F-11D2-BA4B-00A0C93EC93B");
    }
    if (key == QLatin1String("bios-boot")) {
        return QStringLiteral("21686148-6449-6E6F-744E-656564454649");
    }
    const QString prefix = QStringLiteral("guid:");
    if (key.startsWith(prefix) && type.trimmed().size() > prefix.size()) {
        return type.trimmed().mid(prefix.size()).toUpper();
    }
    return QStringLiteral("—");
}

QString sectorText(quint64 bytes, quint32 sectorSize) {
    if (sectorSize == 0) {
        return QStringLiteral("—");
    }
    return QString::number(bytes / sectorSize);
}

QString attributesText(quint64 attributes) {
    const QString digits =
        QStringLiteral("%1").arg(static_cast<qulonglong>(attributes), 16, 16, QLatin1Char('0')).toUpper();
    return QStringLiteral("0x") + digits;
}

QLineEdit *readOnlyField(QWidget *parent) {
    auto *field = new QLineEdit(parent);
    field->setReadOnly(true);
    return field;
}

QCheckBox *displayCheck(const QString &text, QWidget *parent) {
    auto *box = new QCheckBox(text, parent);
    box->setFocusPolicy(Qt::NoFocus);
    box->setAttribute(Qt::WA_TransparentForMouseEvents);
    return box;
}

void setPlaceholderColor(QLabel *label) {
    QPalette palette = label->palette();
    palette.setColor(QPalette::WindowText, palette.color(QPalette::PlaceholderText));
    label->setPalette(palette);
}

} // namespace

PartitionEditorWidget::PartitionEditorWidget(QWidget *parent)
    : QWidget(parent) {
    setObjectName(QStringLiteral("partitionEditor"));
    setAutoFillBackground(true);
    setMinimumWidth(300);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);

    m_name = readOnlyField(this);
    m_type = readOnlyField(this);
    m_typeGuid = readOnlyField(this);
    m_uniqueGuid = readOnlyField(this);
    m_startSector = readOnlyField(this);
    m_endSector = readOnlyField(this);
    m_sectorCount = readOnlyField(this);
    m_attributes = readOnlyField(this);
    m_required = displayCheck(QStringLiteral("平台必需"), this);
    m_noBlockIo = displayCheck(QStringLiteral("固件不提供块设备"), this);
    m_legacyBios = displayCheck(QStringLiteral("传统 BIOS 可引导"), this);

    m_startMode = new QLabel(this);
    m_startMode->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    setPlaceholderColor(m_startMode);

    auto *startRow = new QWidget(this);
    auto *startLayout = new QHBoxLayout(startRow);
    startLayout->setContentsMargins(0, 0, 0, 0);
    startLayout->setSpacing(8);
    startLayout->addWidget(m_startSector, 1);
    startLayout->addWidget(m_startMode);

    auto *formHost = new QWidget(this);
    auto *formHostLayout = new QVBoxLayout(formHost);
    formHostLayout->setContentsMargins(12, 12, 12, 12);
    auto *form = new QFormLayout;
    form->setFieldGrowthPolicy(QFormLayout::ExpandingFieldsGrow);
    form->setLabelAlignment(Qt::AlignRight | Qt::AlignVCenter);
    form->setFormAlignment(Qt::AlignTop);
    form->setHorizontalSpacing(12);
    form->setVerticalSpacing(8);
    form->addRow(QStringLiteral("名称"), m_name);
    form->addRow(QStringLiteral("类型"), m_type);
    form->addRow(QStringLiteral("类型 GUID"), m_typeGuid);
    form->addRow(QStringLiteral("唯一 GUID"), m_uniqueGuid);
    form->addRow(QStringLiteral("起始扇区"), startRow);
    form->addRow(QStringLiteral("结束扇区"), m_endSector);
    form->addRow(QStringLiteral("扇区数"), m_sectorCount);
    form->addRow(QStringLiteral("属性"), m_attributes);
    form->addRow(QString(), m_required);
    form->addRow(QString(), m_noBlockIo);
    form->addRow(QString(), m_legacyBios);
    formHostLayout->addLayout(form);
    formHostLayout->addStretch(1);

    auto *placeholder = new QLabel(QStringLiteral("未选择分区"), this);
    placeholder->setObjectName(QStringLiteral("partitionEditorPlaceholder"));
    placeholder->setAlignment(Qt::AlignCenter);
    placeholder->setWordWrap(true);
    placeholder->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    setPlaceholderColor(placeholder);

    auto *gptScroll = new QScrollArea(this);
    gptScroll->setObjectName(QStringLiteral("partitionEditorGptForm"));
    gptScroll->setWidgetResizable(true);
    gptScroll->setFrameShape(QFrame::NoFrame);
    gptScroll->setWidget(formHost);

    m_gptStack = new QStackedWidget(this);
    m_gptStack->setObjectName(QStringLiteral("partitionEditorGptStack"));
    m_gptStack->addWidget(placeholder);
    m_gptStack->addWidget(gptScroll);

    auto *content = new QWidget(this);
    content->setObjectName(QStringLiteral("partitionEditorContent"));

    m_tabs = new QTabWidget(this);
    m_tabs->setObjectName(QStringLiteral("partitionEditorTabs"));
    m_tabs->addTab(m_gptStack, QStringLiteral("GPT"));
    m_tabs->addTab(content, QStringLiteral("内容"));

    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    layout->addWidget(m_tabs);
}

void PartitionEditorWidget::clearPartition() {
    m_gptStack->setCurrentIndex(0);
    m_name->clear();
    m_type->clear();
    m_typeGuid->clear();
    m_uniqueGuid->clear();
    m_startSector->clear();
    m_startMode->clear();
    m_endSector->clear();
    m_sectorCount->clear();
    m_attributes->clear();
    m_required->setChecked(false);
    m_noBlockIo->setChecked(false);
    m_legacyBios->setChecked(false);
}

void PartitionEditorWidget::setPartition(const PartitionView &partition, quint32 sectorSize) {
    m_name->setText(partition.name);
    m_name->setCursorPosition(0);
    m_type->setText(partition.type);
    m_type->setCursorPosition(0);
    m_typeGuid->setText(typeGuidText(partition.type));
    m_typeGuid->setCursorPosition(0);
    m_uniqueGuid->setText(partition.id);
    m_uniqueGuid->setCursorPosition(0);
    m_startSector->setText(sectorText(partition.startBytes, sectorSize));
    m_startMode->setText(partition.startFixed ? QStringLiteral("固定") : QStringLiteral("自动"));
    if (partition.hasSizeBytes) {
        m_sectorCount->setText(sectorText(partition.sizeBytes, sectorSize));
        if (sectorSize == 0 || partition.sizeBytes < sectorSize) {
            m_endSector->setText(QStringLiteral("—"));
        } else {
            const quint64 start = partition.startBytes / sectorSize;
            const quint64 count = partition.sizeBytes / sectorSize;
            m_endSector->setText(count == 0 ? QStringLiteral("—") : QString::number(start + count - 1));
        }
    } else {
        m_endSector->setText(QStringLiteral("—"));
        m_sectorCount->setText(QStringLiteral("—"));
    }
    m_attributes->setText(attributesText(partition.attributes));
    m_required->setChecked((partition.attributes & kGptRequired) != 0);
    m_noBlockIo->setChecked((partition.attributes & kGptNoBlockIo) != 0);
    m_legacyBios->setChecked((partition.attributes & kGptLegacyBios) != 0);
    m_gptStack->setCurrentIndex(1);
}
