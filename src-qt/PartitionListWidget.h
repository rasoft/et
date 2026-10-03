#pragma once

#include <QTableView>

class PartitionListWidget : public QTableView {
    Q_OBJECT

public:
    enum Column {
        Name = 0,
        StartSector,
        SectorCount,
        Capacity,
        Utilization,
        ColumnCount
    };

    explicit PartitionListWidget(QWidget *parent = nullptr);

    bool hasSelection() const;

signals:
    void selectionChanged(bool hasSelection);
};
