#pragma once

#include <QDialog>

class QComboBox;
class QLineEdit;

class NewPackageDialog : public QDialog {
public:
    explicit NewPackageDialog(QWidget *parent = nullptr);

    QString packageName() const;
    QString directory() const;
    quint64 userAreaBytes() const;
    quint32 sectorSize() const;

protected:
    void accept() override;

private:
    void updateSuggestedPath();
    void browse();
    bool applyForm(QString *error);

    QLineEdit *m_name = nullptr;
    QLineEdit *m_capacity = nullptr;
    QComboBox *m_unit = nullptr;
    QComboBox *m_sector = nullptr;
    QLineEdit *m_path = nullptr;
    QString m_parent;
    bool m_pathEdited = false;

    QString m_packageName;
    QString m_directory;
    quint64 m_userAreaBytes = 0;
    quint32 m_sectorSize = 512;
};
