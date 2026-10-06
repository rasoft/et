#pragma once

#include "EtSession.h"

#include <QDialog>
#include <QVector>

class QCheckBox;
class QTableWidget;
class QTableWidgetItem;

class ImportChoicesDialog : public QDialog {
public:
    explicit ImportChoicesDialog(const QString &sourceName, const ImportPreview &preview,
                                 const QVector<PartitionView> &currentPartitions,
                                 QWidget *parent = nullptr);

    QString selectionJson() const;

protected:
    void accept() override;

private:
    struct ImageChoiceRow {
        quint32 index = 0;
        bool fileMissing = false;
        QString fileMessage;
        bool partitionMissing = false;
        QString tooSmallMessage;
        bool preferred = true;
        QCheckBox *check = nullptr;
        QTableWidgetItem *note = nullptr;
    };

    void refreshImageChoices();
    void updateImageHeaderCheck();
    void applyImageHeaderCheck();
    QString blockReason(const ImageChoiceRow &row) const;

    QCheckBox *m_importTable = nullptr;
    QCheckBox *m_imageHeaderCheck = nullptr;
    QVector<ImageChoiceRow> m_imageRows;
    bool m_refreshing = false;
    QString m_selectionJson;
};
