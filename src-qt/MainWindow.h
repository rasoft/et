#pragma once

#include <QMainWindow>
#include <QString>

class PartitionEditorWidget;
class PartitionListWidget;
class QAction;
class QLabel;
class QWidget;
struct DocumentView;

class MainWindow : public QMainWindow {
    Q_OBJECT

public:
    explicit MainWindow(QWidget *parent = nullptr);

private slots:
    void newPackage();
    void importPackage();
    void savePackage();
    void newPartition();
    void deletePartition();
    void download();
    void onPartitionSelectionChanged(bool hasSelection);

private:
    void createActions();
    void createMenus();
    void createToolBar();
    void createCentralWidget();
    void createStatusBar();
    void updateActionStates();
    void showPending(const QString &command);
    void applyDocument(const DocumentView &view);

    QAction *m_newPackage = nullptr;
    QAction *m_importPackage = nullptr;
    QAction *m_savePackage = nullptr;
    QAction *m_quit = nullptr;
    QAction *m_newPartition = nullptr;
    QAction *m_deletePartition = nullptr;
    QAction *m_download = nullptr;

    PartitionListWidget *m_partitionList = nullptr;
    PartitionEditorWidget *m_partitionEditor = nullptr;
    QWidget *m_openSummary = nullptr;
    QLabel *m_closedSummary = nullptr;
    QLabel *m_nameValue = nullptr;
    QLabel *m_capacityValue = nullptr;
    QLabel *m_sectorValue = nullptr;
    QLabel *m_alignmentValue = nullptr;
    QLabel *m_stateValue = nullptr;
    bool m_hasDocument = false;
    bool m_dirty = false;
};
