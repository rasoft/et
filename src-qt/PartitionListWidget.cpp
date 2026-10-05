#include "PartitionListWidget.h"

#include <QAbstractItemView>
#include <QApplication>
#include <QCheckBox>
#include <QHeaderView>
#include <QItemSelectionModel>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QPainter>
#include <QResizeEvent>
#include <QScrollBar>
#include <QShowEvent>
#include <QStandardItem>
#include <QStandardItemModel>
#include <QStyle>
#include <QStyledItemDelegate>

namespace {

void drawCheckBox(QPainter *painter, const QRect &rect, Qt::CheckState state) {
    painter->save();
    painter->setOpacity(1.0);
    painter->setCompositionMode(QPainter::CompositionMode_SourceOver);
    painter->setRenderHint(QPainter::Antialiasing, true);
    const QRectF box = QRectF(rect).adjusted(0.5, 0.5, -0.5, -0.5);
    const QColor border = state == Qt::Checked ? QColor(0x0a, 0x64, 0xc8) : QColor(0x3a, 0x3a, 0x3a);
    const QColor fill = state == Qt::Checked ? QColor(0x0a, 0x84, 0xff) : QColor(0xff, 0xff, 0xff);
    painter->setPen(QPen(border, 1.4));
    painter->setBrush(fill);
    painter->drawRoundedRect(box, 2.0, 2.0);
    if (state == Qt::Checked) {
        QPen pen(QColor(0xff, 0xff, 0xff), 1.8, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
        painter->setPen(pen);
        const qreal x = box.left();
        const qreal y = box.top();
        const qreal w = box.width();
        const qreal h = box.height();
        painter->drawLine(QPointF(x + w * 0.22, y + h * 0.54), QPointF(x + w * 0.42, y + h * 0.74));
        painter->drawLine(QPointF(x + w * 0.42, y + h * 0.74), QPointF(x + w * 0.78, y + h * 0.32));
    }
    painter->restore();
}

class RowCheckDelegate : public QStyledItemDelegate {
public:
    using QStyledItemDelegate::QStyledItemDelegate;

    void paint(QPainter *painter, const QStyleOptionViewItem &option, const QModelIndex &index) const override {
        QStyleOptionViewItem opt = option;
        initStyleOption(&opt, index);
        opt.features &= ~QStyleOptionViewItem::HasCheckIndicator;
        opt.text.clear();
        opt.icon = QIcon();

        const QWidget *widget = opt.widget;
        QStyle *style = widget != nullptr ? widget->style() : QApplication::style();
        style->drawPrimitive(QStyle::PE_PanelItemViewItem, &opt, painter, widget);

        const QVariant checkData = index.data(Qt::CheckStateRole);
        const auto state = checkData.isValid() ? static_cast<Qt::CheckState>(checkData.toInt()) : Qt::Unchecked;
        const int room = qMin(opt.rect.width(), opt.rect.height());
        if (room < 8) {
            return;
        }
        const int side = qMin(room - 4, 15);
        const QRect box = QStyle::alignedRect(opt.direction, Qt::AlignCenter, QSize(side, side), opt.rect);
        painter->save();
        painter->setClipRect(opt.rect, Qt::ReplaceClip);
        drawCheckBox(painter, box, state);
        painter->restore();
    }

    bool editorEvent(QEvent *event, QAbstractItemModel *model, const QStyleOptionViewItem &option,
                     const QModelIndex &index) override {
        if (event == nullptr || model == nullptr || !index.isValid()) {
            return false;
        }
        if ((model->flags(index) & Qt::ItemIsUserCheckable) == 0) {
            return false;
        }
        if (event->type() == QEvent::MouseButtonRelease) {
            const auto *mouse = static_cast<const QMouseEvent *>(event);
            if (mouse->button() != Qt::LeftButton || !option.rect.contains(mouse->pos())) {
                return false;
            }
        } else if (event->type() == QEvent::KeyPress) {
            const auto *key = static_cast<const QKeyEvent *>(event);
            if (key->key() != Qt::Key_Space && key->key() != Qt::Key_Select) {
                return false;
            }
        } else {
            return false;
        }
        const auto state = static_cast<Qt::CheckState>(index.data(Qt::CheckStateRole).toInt());
        const auto next = state == Qt::Checked ? Qt::Unchecked : Qt::Checked;
        return model->setData(index, next, Qt::CheckStateRole);
    }
};

class HeaderCheckBox : public QCheckBox {
public:
    using QCheckBox::QCheckBox;

protected:
    void nextCheckState() override {
        const auto *header = qobject_cast<const QHeaderView *>(
            parentWidget() != nullptr ? parentWidget()->parentWidget() : nullptr);
        const auto *view = header != nullptr ? qobject_cast<const QAbstractItemView *>(header->parentWidget())
                                             : nullptr;
        const QAbstractItemModel *model = view != nullptr ? view->model() : nullptr;
        if (model == nullptr || model->rowCount() == 0) {
            setCheckState(Qt::Unchecked);
            return;
        }
        setCheckState(checkState() == Qt::Checked ? Qt::Unchecked : Qt::Checked);
    }
};

class PartitionHeaderView : public QHeaderView {
public:
    explicit PartitionHeaderView(QWidget *parent)
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

PartitionHeaderView *headerView(QTableView *view) {
    return static_cast<PartitionHeaderView *>(view->horizontalHeader());
}

} // namespace

PartitionListWidget::PartitionListWidget(QWidget *parent)
    : QTableView(parent) {
    auto *model = new QStandardItemModel(this);
    model->setColumnCount(ColumnCount);
    model->setHorizontalHeaderLabels({
        QString(),
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
    setItemDelegateForColumn(Selected, new RowCheckDelegate(this));
    verticalHeader()->setVisible(false);
    auto *header = new PartitionHeaderView(this);
    setHorizontalHeader(header);
    header->setHighlightSections(false);
    header->setStretchLastSection(false);
    header->setSectionResizeMode(Selected, QHeaderView::Fixed);
    header->setMinimumSectionSize(28);
    setColumnWidth(Selected, 36);
    header->setSectionResizeMode(Name, QHeaderView::Stretch);
    header->setSectionResizeMode(StartSector, QHeaderView::ResizeToContents);
    header->setSectionResizeMode(SectorCount, QHeaderView::ResizeToContents);
    header->setSectionResizeMode(Capacity, QHeaderView::ResizeToContents);
    header->setSectionResizeMode(Utilization, QHeaderView::ResizeToContents);
    header->updateCheckGeometry();

    connect(selectionModel(), &QItemSelectionModel::selectionChanged, this,
            [this] { emit selectionChanged(hasSelection()); });
    connect(model, &QStandardItemModel::itemChanged, this, [this](QStandardItem *item) {
        if (m_updatingChecks || item == nullptr || item->column() != Selected) {
            return;
        }
        updateHeaderCheck();
    });
    connect(header->checkBox(), &QCheckBox::clicked, this, [this](bool) { applyHeaderCheck(); });
    connect(header, &QHeaderView::sectionResized, this, [header](int, int, int) { header->updateCheckGeometry(); });
    connect(horizontalScrollBar(), &QScrollBar::valueChanged, this,
            [header](int) { header->updateCheckGeometry(); });
}

bool PartitionListWidget::hasSelection() const {
    return selectionModel() != nullptr && selectionModel()->hasSelection();
}

QString PartitionListWidget::selectedPartitionId() const {
    const auto *items = qobject_cast<const QStandardItemModel *>(model());
    const QItemSelectionModel *selection = selectionModel();
    if (items == nullptr || selection == nullptr) {
        return {};
    }
    const QModelIndexList rows = selection->selectedRows(Name);
    if (rows.isEmpty()) {
        return {};
    }
    const QStandardItem *name = items->item(rows.first().row(), Name);
    if (name == nullptr) {
        return {};
    }
    return name->data(Qt::UserRole).toString();
}

bool PartitionListWidget::hasChecked() const {
    return !checkedPartitionIds().isEmpty();
}

QStringList PartitionListWidget::checkedPartitionIds() const {
    QStringList ids;
    const auto *items = qobject_cast<const QStandardItemModel *>(model());
    if (items == nullptr) {
        return ids;
    }
    ids.reserve(items->rowCount());
    for (int row = 0; row < items->rowCount(); ++row) {
        const QStandardItem *check = items->item(row, Selected);
        const QStandardItem *name = items->item(row, Name);
        if (check == nullptr || name == nullptr || check->checkState() != Qt::Checked) {
            continue;
        }
        const QString id = name->data(Qt::UserRole).toString();
        if (!id.isEmpty()) {
            ids.append(id);
        }
    }
    return ids;
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
        auto *selected = new QStandardItem;
        selected->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable | Qt::ItemIsUserCheckable);
        selected->setCheckState(Qt::Unchecked);
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
        items->appendRow({selected, name, start, count, capacity, utilization});
    }
    updateHeaderCheck();
}

void PartitionListWidget::updateHeaderCheck() {
    PartitionHeaderView *header = headerView(this);
    auto *items = qobject_cast<QStandardItemModel *>(model());
    if (header == nullptr || header->checkBox() == nullptr || items == nullptr) {
        return;
    }
    int checked = 0;
    const int total = items->rowCount();
    for (int row = 0; row < total; ++row) {
        const QStandardItem *check = items->item(row, Selected);
        if (check != nullptr && check->checkState() == Qt::Checked) {
            ++checked;
        }
    }
    Qt::CheckState state = Qt::Unchecked;
    if (total > 0 && checked == total) {
        state = Qt::Checked;
    } else if (checked > 0) {
        state = Qt::PartiallyChecked;
    }
    if (header->checkBox()->checkState() != state) {
        header->checkBox()->setCheckState(state);
    }
    emit checksChanged(checked > 0);
}

void PartitionListWidget::setAllRowChecks(Qt::CheckState state) {
    auto *items = qobject_cast<QStandardItemModel *>(model());
    if (items == nullptr) {
        return;
    }
    m_updatingChecks = true;
    for (int row = 0; row < items->rowCount(); ++row) {
        QStandardItem *check = items->item(row, Selected);
        if (check != nullptr && check->checkState() != state) {
            check->setCheckState(state);
        }
    }
    m_updatingChecks = false;
    updateHeaderCheck();
}

void PartitionListWidget::applyHeaderCheck() {
    PartitionHeaderView *header = headerView(this);
    if (header == nullptr || header->checkBox() == nullptr) {
        return;
    }
    setAllRowChecks(header->checkBox()->checkState() == Qt::Checked ? Qt::Checked : Qt::Unchecked);
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
