#pragma once

#include <QWidget>

class QLabel;

class PartitionEditorWidget : public QWidget {
public:
    explicit PartitionEditorWidget(QWidget *parent = nullptr);

    void setSelectionAvailable(bool available);

private:
    QLabel *m_placeholder = nullptr;
};
