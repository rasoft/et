#include "PartitionListWidget.h"

#include <QHeaderView>
#include <QItemSelectionModel>
#include <QStandardItem>
#include <QStandardItemModel>

PartitionListWidget::PartitionListWidget(QWidget *parent)
    : QTableView(parent) {
    auto *model = new QStandardItemModel(this);
    model->setColumnCount(ColumnCount);
    model->setHorizontalHeaderLabels({
        QStringLiteral("名字"),
        QStringLiteral("起始扇区"),
        QStringLiteral("扇区数"),
        QStringLiteral("容量"),
        QStringLiteral("利用率"),
    });
    const int alignRight = Qt::AlignRight | Qt::AlignVCenter;
    model->setHeaderData(StartSector, Qt::Horizontal, alignRight, Qt::TextAlignmentRole);
    model->setHeaderData(SectorCount, Qt::Horizontal, alignRight, Qt::TextAlignmentRole);
    model->setHeaderData(Capacity, Qt::Horizontal, alignRight, Qt::TextAlignmentRole);
    model->setHeaderData(Utilization, Qt::Horizontal, alignRight, Qt::TextAlignmentRole);
    setModel(model);

    setObjectName(QStringLiteral("partitionList"));
    setSelectionBehavior(QAbstractItemView::SelectRows);
    setSelectionMode(QAbstractItemView::SingleSelection);
    setEditTriggers(QAbstractItemView::NoEditTriggers);
    setAlternatingRowColors(true);
    setWordWrap(false);
    setTextElideMode(Qt::ElideRight);
    setHorizontalScrollMode(QAbstractItemView::ScrollPerPixel);
    setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
    setMinimumWidth(480);
    verticalHeader()->setVisible(false);
    horizontalHeader()->setHighlightSections(false);
    horizontalHeader()->setStretchLastSection(false);
    horizontalHeader()->setSectionResizeMode(Name, QHeaderView::Stretch);
    horizontalHeader()->setSectionResizeMode(StartSector, QHeaderView::ResizeToContents);
    horizontalHeader()->setSectionResizeMode(SectorCount, QHeaderView::ResizeToContents);
    horizontalHeader()->setSectionResizeMode(Capacity, QHeaderView::ResizeToContents);
    horizontalHeader()->setSectionResizeMode(Utilization, QHeaderView::ResizeToContents);

    connect(selectionModel(), &QItemSelectionModel::selectionChanged, this,
            [this] { emit selectionChanged(hasSelection()); });
}

bool PartitionListWidget::hasSelection() const {
    return selectionModel() != nullptr && selectionModel()->hasSelection();
}

void PartitionListWidget::clearPartitions() {
    setPartitions({});
}

void PartitionListWidget::setPartitions(const QVector<PartitionRow> &rows) {
    auto *items = qobject_cast<QStandardItemModel *>(model());
    if (items == nullptr) {
        return;
    }
    items->removeRows(0, items->rowCount());
    const Qt::Alignment alignRight = Qt::AlignRight | Qt::AlignVCenter;
    for (const PartitionRow &row : rows) {
        auto *name = new QStandardItem(row.name);
        name->setData(row.id, Qt::UserRole);
        auto *start = new QStandardItem(row.startSector);
        auto *count = new QStandardItem(row.sectorCount);
        auto *capacity = new QStandardItem(row.capacity);
        auto *utilization = new QStandardItem(row.utilization);
        start->setTextAlignment(alignRight);
        count->setTextAlignment(alignRight);
        capacity->setTextAlignment(alignRight);
        utilization->setTextAlignment(alignRight);
        items->appendRow({name, start, count, capacity, utilization});
    }
}

void PartitionListWidget::selectPartition(const QString &id) {
    auto *items = qobject_cast<QStandardItemModel *>(model());
    if (items == nullptr || id.isEmpty()) {
        return;
    }
    for (int row = 0; row < items->rowCount(); ++row) {
        QStandardItem *name = items->item(row, Name);
        if (name != nullptr && name->data(Qt::UserRole).toString() == id) {
            selectRow(row);
            return;
        }
    }
}
