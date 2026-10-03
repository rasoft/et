#include "PartitionEditorWidget.h"

#include <QLabel>
#include <QVBoxLayout>

PartitionEditorWidget::PartitionEditorWidget(QWidget *parent)
    : QWidget(parent) {
    setObjectName(QStringLiteral("partitionEditor"));
    setAutoFillBackground(true);
    setMinimumWidth(280);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);

    m_placeholder = new QLabel(QStringLiteral("未选择分区"), this);
    m_placeholder->setAlignment(Qt::AlignCenter);
    m_placeholder->setWordWrap(true);
    m_placeholder->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    QPalette palette = m_placeholder->palette();
    palette.setColor(QPalette::WindowText, palette.color(QPalette::PlaceholderText));
    m_placeholder->setPalette(palette);

    auto *layout = new QVBoxLayout(this);
    layout->addWidget(m_placeholder);
}

void PartitionEditorWidget::setSelectionAvailable(bool available) {
    m_placeholder->setVisible(!available);
}
