#include "MainWindow.h"

#include "EtSession.h"
#include "NewPackageDialog.h"
#include "PartitionEditorWidget.h"
#include "PartitionListWidget.h"
#include "ToolbarIcons.h"

#include <QAction>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QMenu>
#include <QMenuBar>
#include <QMessageBox>
#include <QPushButton>
#include <QSplitter>
#include <QStatusBar>
#include <QToolBar>
#include <QVBoxLayout>

namespace {

QString formatBytes(quint64 bytes) {
    const quint64 kib = 1024;
    const quint64 mib = kib * 1024;
    const quint64 gib = mib * 1024;
    const quint64 tib = gib * 1024;
    if (bytes >= tib && bytes % tib == 0) {
        return QStringLiteral("%1 TiB").arg(bytes / tib);
    }
    if (bytes >= gib && bytes % gib == 0) {
        return QStringLiteral("%1 GiB").arg(bytes / gib);
    }
    if (bytes >= mib && bytes % mib == 0) {
        return QStringLiteral("%1 MiB").arg(bytes / mib);
    }
    if (bytes >= kib && bytes % kib == 0) {
        return QStringLiteral("%1 KiB").arg(bytes / kib);
    }
    return QStringLiteral("%1 字节").arg(bytes);
}

void showWarning(QWidget *parent, const QString &text) {
    QMessageBox box(parent);
    box.setIcon(QMessageBox::Warning);
    box.setWindowTitle(QStringLiteral("新建镜像包"));
    box.setText(text);
    box.addButton(QStringLiteral("确定"), QMessageBox::AcceptRole);
    box.exec();
}

} // namespace

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

    m_newPackage->setIcon(toolbarIcon(ToolbarIcon::NewPackage));
    m_savePackage->setIcon(toolbarIcon(ToolbarIcon::Save));
    m_deletePartition->setIcon(toolbarIcon(ToolbarIcon::DeletePartition));
    m_newPartition->setIcon(toolbarIcon(ToolbarIcon::AddPartition));
    m_download->setIcon(toolbarIcon(ToolbarIcon::Download));

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
    toolBar->setToolButtonStyle(Qt::ToolButtonTextUnderIcon);
    toolBar->setIconSize(QSize(24, 24));
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

    auto *bar = new QWidget(this);
    auto *barLayout = new QVBoxLayout(bar);
    barLayout->setContentsMargins(0, 0, 0, 0);
    barLayout->setSpacing(0);

    auto *rowHost = new QWidget(bar);
    auto *row = new QHBoxLayout(rowHost);
    row->setContentsMargins(12, 8, 12, 8);
    m_closedSummary = new QLabel(QStringLiteral("未打开镜像包"), rowHost);
    QPalette closedPalette = m_closedSummary->palette();
    closedPalette.setColor(QPalette::WindowText,
                            closedPalette.color(QPalette::Disabled, QPalette::WindowText));
    m_closedSummary->setPalette(closedPalette);
    row->addWidget(m_closedSummary);

    m_openSummary = new QWidget(rowHost);
    auto *openLayout = new QHBoxLayout(m_openSummary);
    openLayout->setContentsMargins(0, 0, 0, 0);
    openLayout->setSpacing(6);
    const auto addField = [this, openLayout](const QString &caption, QLabel **value) {
        auto *label = new QLabel(caption, m_openSummary);
        QPalette palette = label->palette();
        palette.setColor(QPalette::WindowText, palette.color(QPalette::Disabled, QPalette::WindowText));
        label->setPalette(palette);
        *value = new QLabel(m_openSummary);
        openLayout->addWidget(label);
        openLayout->addWidget(*value);
        openLayout->addSpacing(16);
    };
    addField(QStringLiteral("名称"), &m_nameValue);
    addField(QStringLiteral("容量"), &m_capacityValue);
    addField(QStringLiteral("扇区"), &m_sectorValue);
    addField(QStringLiteral("对齐"), &m_alignmentValue);
    addField(QStringLiteral("状态"), &m_stateValue);
    openLayout->addStretch(1);
    m_openSummary->setVisible(false);
    row->addWidget(m_openSummary, 1);

    auto *line = new QFrame(bar);
    line->setFrameShape(QFrame::HLine);
    line->setFrameShadow(QFrame::Sunken);
    barLayout->addWidget(rowHost);
    barLayout->addWidget(line);

    auto *central = new QWidget(this);
    auto *layout = new QVBoxLayout(central);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    layout->addWidget(bar);
    layout->addWidget(splitter, 1);
    setCentralWidget(central);

    connect(m_partitionList, &PartitionListWidget::selectionChanged, this,
            &MainWindow::onPartitionSelectionChanged);
}

void MainWindow::createStatusBar() {
    statusBar()->showMessage(QStringLiteral("就绪"));
}

void MainWindow::updateActionStates() {
    const bool hasSelection = m_partitionList != nullptr && m_partitionList->hasSelection();
    m_savePackage->setEnabled(m_hasDocument);
    m_newPartition->setEnabled(m_hasDocument);
    m_deletePartition->setEnabled(m_hasDocument && hasSelection);
    m_download->setEnabled(m_hasDocument);
}

void MainWindow::showPending(const QString &command) {
    statusBar()->showMessage(QStringLiteral("「%1」尚未实现").arg(command), 3000);
}

void MainWindow::newPackage() {
    bool discardUnsaved = false;
    if (m_hasDocument && m_dirty) {
        QMessageBox box(this);
        box.setIcon(QMessageBox::Question);
        box.setWindowTitle(QStringLiteral("新建镜像包"));
        box.setText(QStringLiteral("当前镜像包有未保存的修改。要放弃这些修改并新建吗？"));
        auto *discard = box.addButton(QStringLiteral("放弃修改"), QMessageBox::AcceptRole);
        auto *cancel = box.addButton(QStringLiteral("取消"), QMessageBox::RejectRole);
        box.setDefaultButton(qobject_cast<QPushButton *>(cancel));
        box.exec();
        if (box.clickedButton() != discard) {
            return;
        }
        discardUnsaved = true;
    }

    NewPackageDialog dialog(this);
    while (dialog.exec() == QDialog::Accepted) {
        QString viewJson;
        QString error;
        if (!EtSession::createPackage(dialog.directory(), dialog.packageName(), dialog.userAreaBytes(),
                                      dialog.sectorSize(), discardUnsaved, &viewJson, &error)) {
            showWarning(this, error);
            continue;
        }
        DocumentView view;
        if (!DocumentView::parse(viewJson, &view, &error)) {
            showWarning(this, QStringLiteral("已创建 %1，但界面没能读回结果：%2")
                                  .arg(dialog.directory(), error));
            return;
        }
        applyDocument(view);
        return;
    }
}

void MainWindow::applyDocument(const DocumentView &view) {
    m_hasDocument = true;
    m_dirty = view.dirty;
    m_closedSummary->setVisible(false);
    m_openSummary->setVisible(true);
    m_nameValue->setText(view.name);
    m_nameValue->setToolTip(view.description.isEmpty() ? view.name : view.description);
    m_capacityValue->setText(formatBytes(view.userAreaBytes));
    m_sectorValue->setText(QStringLiteral("%1 字节").arg(view.sectorSize));
    m_alignmentValue->setText(formatBytes(view.alignment));
    QString state = view.dirty ? QStringLiteral("未保存") : QStringLiteral("已保存");
    if (view.issueCount > 0) {
        state += QStringLiteral("，含错误");
    }
    m_stateValue->setText(state);
    setWindowTitle(view.dirty ? QStringLiteral("%1* — et").arg(view.name)
                              : QStringLiteral("%1 — et").arg(view.name));
    m_partitionList->clearPartitions();
    m_partitionEditor->setSelectionAvailable(false);
    statusBar()->showMessage(view.root);
    updateActionStates();
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
