#pragma once

#include <QString>
#include <QTableView>
#include <QVector>

struct PartitionRow {
    QString id;
    QString name;
    QString startSector;
    QString sectorCount;
    QString capacity;
    QString utilization;
};

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
    void clearPartitions();
    void setPartitions(const QVector<PartitionRow> &rows);
    void selectPartition(const QString &id);

signals:
    void selectionChanged(bool hasSelection);
};
