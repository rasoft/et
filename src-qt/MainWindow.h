#pragma once

#include "EtSession.h"

#include <QMainWindow>
#include <QString>

class QCloseEvent;
class PartitionEditorWidget;
class PartitionListWidget;
class QAction;
class QLabel;
class QListWidget;
class QWidget;

class MainWindow : public QMainWindow {
    Q_OBJECT

public:
    explicit MainWindow(QWidget *parent = nullptr);

protected:
    void closeEvent(QCloseEvent *event) override;

private slots:
    void newPackage();
    void openPackage();
    void importPackage();
    void savePackage();
    void newPartition();
    void deletePartition();
    void download();
    void onPartitionSelectionChanged(bool hasSelection);
    void onPartitionChecksChanged();

private:
    void createActions();
    void createMenus();
    void createToolBar();
    void createCentralWidget();
    void createStatusBar();
    void updateActionStates();
    void showPending(const QString &command);
    void applyDocument(const DocumentView &view);
    bool confirmReplace(const QString &title, const QString &question, bool *discardUnsaved);
    bool confirmSaveOrDiscard(const QString &title);
    bool saveToArchive(const QString &path);
    QString askArchivePath();
    QString browseDirectory() const;
    void showWarning(const QString &title, const QString &text);

    QAction *m_newPackage = nullptr;
    QAction *m_openPackage = nullptr;
    QAction *m_importPackage = nullptr;
    QAction *m_savePackage = nullptr;
    QAction *m_quit = nullptr;
    QAction *m_newPartition = nullptr;
    QAction *m_deletePartition = nullptr;
    QAction *m_download = nullptr;

    PartitionListWidget *m_partitionList = nullptr;
    PartitionEditorWidget *m_partitionEditor = nullptr;
    QVector<PartitionView> m_partitions;
    quint32 m_sectorSize = 512;
    QWidget *m_openSummary = nullptr;
    QLabel *m_closedSummary = nullptr;
    QLabel *m_nameValue = nullptr;
    QLabel *m_capacityValue = nullptr;
    QLabel *m_sectorValue = nullptr;
    QLabel *m_alignmentValue = nullptr;
    QLabel *m_stateValue = nullptr;
    QListWidget *m_issueList = nullptr;
    QString m_archivePath;
    bool m_hasDocument = false;
    bool m_dirty = false;
};
