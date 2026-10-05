#pragma once

#include <QString>
#include <QStringList>
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
        Selected = 0,
        Name,
        StartSector,
        SectorCount,
        Capacity,
        Utilization,
        ColumnCount
    };

    explicit PartitionListWidget(QWidget *parent = nullptr);

    bool hasSelection() const;
    QString selectedPartitionId() const;
    bool hasChecked() const;
    QStringList checkedPartitionIds() const;
    void clearPartitions();
    void setPartitions(const QVector<PartitionRow> &rows);
    void selectPartition(const QString &id);

signals:
    void selectionChanged(bool hasSelection);
    void checksChanged(bool anyChecked);

private:
    void updateHeaderCheck();
    void setAllRowChecks(Qt::CheckState state);
    void applyHeaderCheck();

    bool m_updatingChecks = false;
};
