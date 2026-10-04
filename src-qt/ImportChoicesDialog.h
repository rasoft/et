#pragma once

#include "EtSession.h"

#include <QDialog>

class QCheckBox;
class QTableWidget;

class ImportChoicesDialog : public QDialog {
public:
    explicit ImportChoicesDialog(const QString &sourceName, const ImportPreview &preview,
                                 QWidget *parent = nullptr);

    QString selectionJson() const;

protected:
    void accept() override;

private:
    QCheckBox *m_importTable = nullptr;
    QTableWidget *m_images = nullptr;
    QString m_selectionJson;
};
