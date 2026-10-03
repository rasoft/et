#pragma once

#include <QMainWindow>
#include <QString>

class PartitionEditorWidget;
class PartitionListWidget;
class QAction;

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

    QAction *m_newPackage = nullptr;
    QAction *m_importPackage = nullptr;
    QAction *m_savePackage = nullptr;
    QAction *m_quit = nullptr;
    QAction *m_newPartition = nullptr;
    QAction *m_deletePartition = nullptr;
    QAction *m_download = nullptr;

    PartitionListWidget *m_partitionList = nullptr;
    PartitionEditorWidget *m_partitionEditor = nullptr;
    bool m_packageOpen = false;
};
