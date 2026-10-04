#pragma once

#include <QDialog>
#include <QString>

class QLineEdit;
class QWidget;

class ImportPackageDialog : public QDialog {
public:
    explicit ImportPackageDialog(const QString &sourceFile, QWidget *parent = nullptr);

    static QString pickSourceFile(QWidget *parent, const QString &startDirectory);

    QString sourceFile() const;
    QString packageName() const;
    QString directory() const;

protected:
    void accept() override;

private:
    void browseSource();
    void browseDirectory();
    void applySourceDefaults();
    void updateSuggestedPath();
    bool applyForm(QString *error);

    QLineEdit *m_source = nullptr;
    QLineEdit *m_name = nullptr;
    QLineEdit *m_path = nullptr;
    QString m_parent;
    bool m_nameEdited = false;
    bool m_pathEdited = false;

    QString m_sourceFile;
    QString m_packageName;
    QString m_directory;
};
