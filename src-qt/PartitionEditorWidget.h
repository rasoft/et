#pragma once

#include <QWidget>

class QCheckBox;
class QLabel;
class QLineEdit;
class QStackedWidget;
class QTabWidget;
struct PartitionView;

class PartitionEditorWidget : public QWidget {
public:
    explicit PartitionEditorWidget(QWidget *parent = nullptr);

    void clearPartition();
    void setPartition(const PartitionView &partition, quint32 sectorSize);

private:
    QTabWidget *m_tabs = nullptr;
    QStackedWidget *m_gptStack = nullptr;
    QLineEdit *m_name = nullptr;
    QLineEdit *m_type = nullptr;
    QLineEdit *m_typeGuid = nullptr;
    QLineEdit *m_uniqueGuid = nullptr;
    QLineEdit *m_startSector = nullptr;
    QLabel *m_startMode = nullptr;
    QLineEdit *m_endSector = nullptr;
    QLineEdit *m_sectorCount = nullptr;
    QLineEdit *m_attributes = nullptr;
    QCheckBox *m_required = nullptr;
    QCheckBox *m_noBlockIo = nullptr;
    QCheckBox *m_legacyBios = nullptr;
};
