#include "MainWindow.h"

#include "PartitionEditorWidget.h"
#include "PartitionListWidget.h"

#include <QAction>
#include <QMenu>
#include <QMenuBar>
#include <QSplitter>
#include <QStatusBar>
#include <QToolBar>

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent) {
    setWindowTitle(QStringLiteral("et"));
    resize(1280, 800);
    setMinimumSize(1024, 640);

    createActions();
    createMenus();
    createToolBar();
    createCentralWidget();
    createStatusBar();
    updateActionStates();
}

void MainWindow::createActions() {
    m_newPackage = new QAction(QStringLiteral("新建..."), this);
    m_newPackage->setIconText(QStringLiteral("新建"));
    m_newPackage->setShortcut(QKeySequence::New);
    m_newPackage->setStatusTip(QStringLiteral("新建镜像包"));

    m_importPackage = new QAction(QStringLiteral("导入..."), this);
    m_importPackage->setStatusTip(QStringLiteral("导入镜像包"));

    m_savePackage = new QAction(QStringLiteral("保存"), this);
    m_savePackage->setShortcut(QKeySequence::Save);
    m_savePackage->setStatusTip(QStringLiteral("保存当前镜像包"));

    m_quit = new QAction(QStringLiteral("退出"), this);
    m_quit->setMenuRole(QAction::QuitRole);
    m_quit->setShortcut(QKeySequence::Quit);
    m_quit->setStatusTip(QStringLiteral("退出程序"));

    m_newPartition = new QAction(QStringLiteral("新建"), this);
    m_newPartition->setIconText(QStringLiteral("增加分区"));
    m_newPartition->setStatusTip(QStringLiteral("增加分区"));

    m_deletePartition = new QAction(QStringLiteral("删除"), this);
    m_deletePartition->setIconText(QStringLiteral("删除分区"));
    m_deletePartition->setStatusTip(QStringLiteral("删除选中的分区"));

    m_download = new QAction(QStringLiteral("下载"), this);
    m_download->setStatusTip(QStringLiteral("下载到设备"));

    connect(m_newPackage, &QAction::triggered, this, &MainWindow::newPackage);
    connect(m_importPackage, &QAction::triggered, this, &MainWindow::importPackage);
    connect(m_savePackage, &QAction::triggered, this, &MainWindow::savePackage);
    connect(m_quit, &QAction::triggered, this, &QWidget::close);
    connect(m_newPartition, &QAction::triggered, this, &MainWindow::newPartition);
    connect(m_deletePartition, &QAction::triggered, this, &MainWindow::deletePartition);
    connect(m_download, &QAction::triggered, this, &MainWindow::download);
}

void MainWindow::createMenus() {
    auto *fileMenu = menuBar()->addMenu(QStringLiteral("文件"));
    fileMenu->addAction(m_newPackage);
    fileMenu->addAction(m_importPackage);
    fileMenu->addAction(m_savePackage);
    fileMenu->addSeparator();
    fileMenu->addAction(m_quit);

    auto *partitionMenu = menuBar()->addMenu(QStringLiteral("分区"));
    partitionMenu->addAction(m_newPartition);
    partitionMenu->addAction(m_deletePartition);

    menuBar()->addMenu(QStringLiteral("帮助"));
}

void MainWindow::createToolBar() {
    auto *toolBar = addToolBar(QStringLiteral("工具栏"));
    toolBar->setObjectName(QStringLiteral("mainToolBar"));
    toolBar->setMovable(false);
    toolBar->setFloatable(false);
    toolBar->setToolButtonStyle(Qt::ToolButtonTextOnly);
    toolBar->addAction(m_newPackage);
    toolBar->addAction(m_savePackage);
    toolBar->addSeparator();
    toolBar->addAction(m_deletePartition);
    toolBar->addAction(m_newPartition);
    toolBar->addSeparator();
    toolBar->addAction(m_download);
}

void MainWindow::createCentralWidget() {
    m_partitionList = new PartitionListWidget(this);
    m_partitionEditor = new PartitionEditorWidget(this);

    auto *splitter = new QSplitter(Qt::Horizontal, this);
    splitter->setObjectName(QStringLiteral("partitionSplitter"));
    splitter->setChildrenCollapsible(false);
    splitter->addWidget(m_partitionList);
    splitter->addWidget(m_partitionEditor);
    splitter->setStretchFactor(0, 3);
    splitter->setStretchFactor(1, 2);
    splitter->setSizes({760, 420});
    setCentralWidget(splitter);

    connect(m_partitionList, &PartitionListWidget::selectionChanged, this,
            &MainWindow::onPartitionSelectionChanged);
}

void MainWindow::createStatusBar() {
    statusBar()->showMessage(QStringLiteral("就绪"));
}

void MainWindow::updateActionStates() {
    const bool hasSelection = m_partitionList != nullptr && m_partitionList->hasSelection();
    m_savePackage->setEnabled(m_packageOpen);
    m_newPartition->setEnabled(m_packageOpen);
    m_deletePartition->setEnabled(m_packageOpen && hasSelection);
    m_download->setEnabled(m_packageOpen);
}

void MainWindow::showPending(const QString &command) {
    statusBar()->showMessage(QStringLiteral("「%1」尚未实现").arg(command), 3000);
}

void MainWindow::newPackage() {
    showPending(QStringLiteral("新建"));
}

void MainWindow::importPackage() {
    showPending(QStringLiteral("导入"));
}

void MainWindow::savePackage() {
    showPending(QStringLiteral("保存"));
}

void MainWindow::newPartition() {
    showPending(QStringLiteral("增加分区"));
}

void MainWindow::deletePartition() {
    showPending(QStringLiteral("删除分区"));
}

void MainWindow::download() {
    showPending(QStringLiteral("下载"));
}

void MainWindow::onPartitionSelectionChanged(bool hasSelection) {
    m_partitionEditor->setSelectionAvailable(hasSelection);
    updateActionStates();
}
