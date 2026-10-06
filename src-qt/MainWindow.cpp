#include "MainWindow.h"

#include "EtSession.h"
#include "ImportChoicesDialog.h"
#include "PartitionEditorWidget.h"
#include "PartitionListWidget.h"
#include "ToolbarIcons.h"

#include <QAction>
#include <QCloseEvent>
#include <QDir>
#include <QFileDialog>
#include <QFileInfo>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QListWidget>
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

QString sectorCountText(quint64 bytes, quint32 sectorSize) {
    if (sectorSize == 0) {
        return QStringLiteral("—");
    }
    return QString::number(bytes / sectorSize);
}

QString utilizationText(const PartitionView &partition) {
    if (!partition.hasImage) {
        return QStringLiteral("—");
    }
    if (!partition.hasImageBytes) {
        return QStringLiteral("缺失");
    }
    if (!partition.hasSizeBytes || partition.sizeBytes == 0) {
        return QStringLiteral("—");
    }
    const quint64 percent = partition.imageBytes * 100 / partition.sizeBytes;
    return QStringLiteral("%1%").arg(percent);
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
    m_newPackage = new QAction(QStringLiteral("新建"), this);
    m_newPackage->setIconText(QStringLiteral("新建"));
    m_newPackage->setShortcut(QKeySequence::New);
    m_newPackage->setStatusTip(QStringLiteral("在临时目录新建镜像包"));

    m_openPackage = new QAction(QStringLiteral("打开..."), this);
    m_openPackage->setIconText(QStringLiteral("打开"));
    m_openPackage->setShortcut(QKeySequence::Open);
    m_openPackage->setStatusTip(QStringLiteral("从 etpk 文件打开镜像包"));

    m_importPackage = new QAction(QStringLiteral("导入..."), this);
    m_importPackage->setIconText(QStringLiteral("导入"));
    m_importPackage->setStatusTip(QStringLiteral("把 flash.conf 或 download.bin 导入当前镜像包"));

    m_savePackage = new QAction(QStringLiteral("保存"), this);
    m_savePackage->setShortcut(QKeySequence::Save);
    m_savePackage->setStatusTip(QStringLiteral("把工作副本打包成 etpk 文件"));

    m_savePackageAs = new QAction(QStringLiteral("另存为..."), this);
    m_savePackageAs->setShortcut(QKeySequence::SaveAs);
    m_savePackageAs->setStatusTip(QStringLiteral("把工作副本打包成另一个 etpk 文件，并打开它"));

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
    m_openPackage->setIcon(toolbarIcon(ToolbarIcon::OpenPackage));
    m_importPackage->setIcon(toolbarIcon(ToolbarIcon::Import));
    m_savePackage->setIcon(toolbarIcon(ToolbarIcon::Save));
    m_deletePartition->setIcon(toolbarIcon(ToolbarIcon::DeletePartition));
    m_newPartition->setIcon(toolbarIcon(ToolbarIcon::AddPartition));
    m_download->setIcon(toolbarIcon(ToolbarIcon::Download));

    connect(m_newPackage, &QAction::triggered, this, &MainWindow::newPackage);
    connect(m_openPackage, &QAction::triggered, this, &MainWindow::openPackage);
    connect(m_importPackage, &QAction::triggered, this, &MainWindow::importPackage);
    connect(m_savePackage, &QAction::triggered, this, &MainWindow::savePackage);
    connect(m_savePackageAs, &QAction::triggered, this, &MainWindow::savePackageAs);
    connect(m_quit, &QAction::triggered, this, &QWidget::close);
    connect(m_newPartition, &QAction::triggered, this, &MainWindow::newPartition);
    connect(m_deletePartition, &QAction::triggered, this, &MainWindow::deletePartition);
    connect(m_download, &QAction::triggered, this, &MainWindow::download);
}

void MainWindow::createMenus() {
    auto *fileMenu = menuBar()->addMenu(QStringLiteral("文件"));
    fileMenu->addAction(m_newPackage);
    fileMenu->addAction(m_openPackage);
    fileMenu->addAction(m_importPackage);
    fileMenu->addAction(m_savePackage);
    fileMenu->addAction(m_savePackageAs);
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
    toolBar->addAction(m_openPackage);
    toolBar->addAction(m_importPackage);
    toolBar->addAction(m_savePackage);
    toolBar->addSeparator();
    toolBar->addAction(m_newPartition);
    toolBar->addAction(m_deletePartition);
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
    m_issueList = new QListWidget(central);
    m_issueList->setObjectName(QStringLiteral("issueList"));
    m_issueList->setMaximumHeight(140);
    m_issueList->setVisible(false);

    layout->addWidget(bar);
    layout->addWidget(splitter, 1);
    layout->addWidget(m_issueList);
    setCentralWidget(central);

    connect(m_issueList, &QListWidget::itemClicked, this, [this](QListWidgetItem *item) {
        if (item == nullptr) {
            return;
        }
        m_partitionList->selectPartition(item->data(Qt::UserRole).toString());
    });

    connect(m_partitionList, &PartitionListWidget::selectionChanged, this,
            &MainWindow::onPartitionSelectionChanged);
    connect(m_partitionList, &PartitionListWidget::checksChanged, this,
            &MainWindow::onPartitionChecksChanged);
}

void MainWindow::createStatusBar() {
    statusBar()->showMessage(QStringLiteral("就绪"));
}

void MainWindow::updateActionStates() {
    const bool hasChecked = m_partitionList != nullptr && m_partitionList->hasChecked();
    m_savePackage->setEnabled(m_hasDocument);
    m_savePackageAs->setEnabled(m_hasDocument);
    m_importPackage->setEnabled(m_hasDocument);
    m_newPartition->setEnabled(m_hasDocument);
    m_deletePartition->setEnabled(m_hasDocument && hasChecked);
    m_download->setEnabled(m_hasDocument);
}

void MainWindow::showPending(const QString &command) {
    statusBar()->showMessage(QStringLiteral("「%1」尚未实现").arg(command), 3000);
}

bool MainWindow::confirmReplace(const QString &title, const QString &question, bool *discardUnsaved) {
    *discardUnsaved = false;
    if (!m_hasDocument || !m_dirty) {
        return true;
    }
    QMessageBox box(this);
    box.setIcon(QMessageBox::Question);
    box.setWindowTitle(title);
    box.setText(question);
    auto *discard = box.addButton(QStringLiteral("放弃修改"), QMessageBox::AcceptRole);
    auto *cancel = box.addButton(QStringLiteral("取消"), QMessageBox::RejectRole);
    box.setDefaultButton(qobject_cast<QPushButton *>(cancel));
    box.exec();
    if (box.clickedButton() != discard) {
        return false;
    }
    *discardUnsaved = true;
    return true;
}

bool MainWindow::confirmSaveOrDiscard(const QString &title) {
    if (!m_hasDocument || !m_dirty) {
        return true;
    }
    QMessageBox box(this);
    box.setIcon(QMessageBox::Question);
    box.setWindowTitle(title);
    box.setText(QStringLiteral("当前镜像包有未保存的修改。要保存吗？"));
    auto *save = box.addButton(QStringLiteral("保存"), QMessageBox::AcceptRole);
    auto *discard = box.addButton(QStringLiteral("不保存"), QMessageBox::DestructiveRole);
    auto *cancel = box.addButton(QStringLiteral("取消"), QMessageBox::RejectRole);
    box.setDefaultButton(qobject_cast<QPushButton *>(save));
    box.setEscapeButton(qobject_cast<QPushButton *>(cancel));
    box.exec();
    if (box.clickedButton() == discard) {
        return true;
    }
    if (box.clickedButton() == save) {
        return saveToArchive(QString());
    }
    return false;
}

QString MainWindow::browseDirectory() const {
    if (!m_archivePath.isEmpty()) {
        const QString directory = QFileInfo(m_archivePath).absolutePath();
        if (!directory.isEmpty()) {
            return directory;
        }
    }
    return QDir::homePath();
}

QString MainWindow::askArchivePath(const QString &title) {
    QString name;
    if (!m_archivePath.isEmpty()) {
        name = QFileInfo(m_archivePath).completeBaseName();
    }
    if (name.isEmpty() && m_nameValue != nullptr) {
        name = m_nameValue->text().trimmed();
    }
    QString safe;
    const QString forbidden = QStringLiteral("/\\:<>\"|?*");
    for (const QChar ch : name) {
        if (ch.unicode() < 0x20 || forbidden.contains(ch)) {
            safe.append(QLatin1Char('_'));
        } else {
            safe.append(ch);
        }
    }
    safe = safe.trimmed();
    if (safe.isEmpty() || safe == QLatin1String(".") || safe == QLatin1String("..")) {
        safe = QStringLiteral("未命名");
    }
    if (!safe.endsWith(QStringLiteral(".etpk"), Qt::CaseInsensitive)) {
        safe.append(QStringLiteral(".etpk"));
    }
    const QString suggested = QDir(browseDirectory()).filePath(safe);
    QString path = QFileDialog::getSaveFileName(this, title, suggested,
                                                QStringLiteral("镜像包 (*.etpk)"), nullptr,
                                                QFileDialog::DontUseNativeDialog);
    if (path.isEmpty()) {
        return {};
    }
    if (!path.endsWith(QStringLiteral(".etpk"), Qt::CaseInsensitive)) {
        path += QStringLiteral(".etpk");
    }
    return path;
}

bool MainWindow::saveToArchive(const QString &path) {
    if (!m_hasDocument) {
        showWarning(QStringLiteral("保存镜像包"), QStringLiteral("请先新建或打开镜像包"));
        return false;
    }
    QString target = path.isEmpty() ? m_archivePath : path;
    if (target.isEmpty()) {
        target = askArchivePath(QStringLiteral("保存镜像包"));
        if (target.isEmpty()) {
            return false;
        }
    }
    QString viewJson;
    QString error;
    if (!EtSession::savePackage(target, &viewJson, &error)) {
        showWarning(QStringLiteral("保存镜像包"), error);
        return false;
    }
    DocumentView view;
    if (!DocumentView::parse(viewJson, &view, &error)) {
        showWarning(QStringLiteral("保存镜像包"),
                    QStringLiteral("已写入 %1，但界面没能读回结果：%2").arg(target, error));
        return false;
    }
    applyDocument(view);
    return true;
}

void MainWindow::showWarning(const QString &title, const QString &text) {
    QMessageBox box(this);
    box.setIcon(QMessageBox::Warning);
    box.setWindowTitle(title);
    box.setText(text);
    box.addButton(QStringLiteral("确定"), QMessageBox::AcceptRole);
    box.exec();
}

void MainWindow::newPackage() {
    if (!confirmSaveOrDiscard(QStringLiteral("新建镜像包"))) {
        return;
    }

    QString viewJson;
    QString error;
    if (!EtSession::createPackage(m_dirty, &viewJson, &error)) {
        showWarning(QStringLiteral("新建镜像包"), error);
        return;
    }
    DocumentView view;
    if (!DocumentView::parse(viewJson, &view, &error)) {
        showWarning(QStringLiteral("新建镜像包"),
                    QStringLiteral("已创建工作副本，但界面没能读回结果：%1").arg(error));
        return;
    }
    applyDocument(view);
}

void MainWindow::openPackage() {
    if (!confirmSaveOrDiscard(QStringLiteral("打开镜像包"))) {
        return;
    }

    const QString archiveFile = QFileDialog::getOpenFileName(
        this, QStringLiteral("打开镜像包"), browseDirectory(),
        QStringLiteral("镜像包 (*.etpk);;所有文件 (*)"), nullptr, QFileDialog::DontUseNativeDialog);
    if (archiveFile.isEmpty()) {
        return;
    }

    QString viewJson;
    QString error;
    if (!EtSession::openPackage(archiveFile, m_dirty, &viewJson, &error)) {
        showWarning(QStringLiteral("打开镜像包"), error);
        return;
    }
    DocumentView view;
    if (!DocumentView::parse(viewJson, &view, &error)) {
        showWarning(QStringLiteral("打开镜像包"),
                    QStringLiteral("已打开 %1，但界面没能读回结果：%2").arg(archiveFile, error));
        return;
    }
    applyDocument(view);
}

void MainWindow::closeEvent(QCloseEvent *event) {
    if (!confirmSaveOrDiscard(QStringLiteral("退出"))) {
        event->ignore();
        return;
    }
    EtSession::closePackage();
    event->accept();
}

void MainWindow::applyDocument(const DocumentView &view) {
    m_hasDocument = true;
    m_dirty = view.dirty;
    m_archivePath = view.archive;
    m_closedSummary->setVisible(false);
    m_openSummary->setVisible(true);
    m_nameValue->setText(view.name);
    m_nameValue->setToolTip(view.description.isEmpty() ? view.name : view.description);
    m_capacityValue->setText(view.hasUserAreaBytes ? formatBytes(view.userAreaBytes)
                                                    : QStringLiteral("下载时确定"));
    m_sectorValue->setText(QStringLiteral("%1 字节").arg(view.sectorSize));
    m_alignmentValue->setText(formatBytes(view.alignment));
    QString state = view.dirty ? QStringLiteral("未保存") : QStringLiteral("已保存");
    if (view.issueCount > 0) {
        state += QStringLiteral("，含错误");
    }
    m_stateValue->setText(state);
    const QString titleName =
        view.archive.isEmpty() ? view.name : QFileInfo(view.archive).fileName();
    setWindowTitle(view.dirty ? QStringLiteral("%1* — et").arg(titleName)
                              : QStringLiteral("%1 — et").arg(titleName));

    QVector<PartitionRow> rows;
    rows.reserve(view.partitions.size());
    for (const PartitionView &partition : view.partitions) {
        PartitionRow row;
        row.id = partition.id;
        row.name = partition.name;
        row.startSector = sectorCountText(partition.startBytes, view.sectorSize);
        if (partition.hasSizeBytes) {
            row.sectorCount = sectorCountText(partition.sizeBytes, view.sectorSize);
            row.capacity = formatBytes(partition.sizeBytes);
        } else {
            row.sectorCount = QStringLiteral("—");
            row.capacity = QStringLiteral("剩余空间");
        }
        row.utilization = utilizationText(partition);
        rows.append(row);
    }
    m_partitions = view.partitions;
    m_sectorSize = view.sectorSize;
    m_partitionList->setPartitions(rows);
    onPartitionSelectionChanged(m_partitionList->hasSelection());

    m_issueList->blockSignals(true);
    m_issueList->clear();
    for (const IssueView &issue : view.issues) {
        auto *item = new QListWidgetItem(issue.message, m_issueList);
        if (issue.hasPartitionId) {
            item->setData(Qt::UserRole, issue.partitionId);
        }
    }
    m_issueList->blockSignals(false);
    m_issueList->setVisible(!view.issues.isEmpty());

    statusBar()->showMessage(view.archive.isEmpty() ? QStringLiteral("尚未保存") : view.archive);
    updateActionStates();
}

void MainWindow::importPackage() {
    if (!m_hasDocument) {
        showWarning(QStringLiteral("导入"), QStringLiteral("请先新建或打开镜像包"));
        return;
    }

    bool discardUnsaved = false;
    if (!confirmReplace(QStringLiteral("导入"),
                        QStringLiteral("当前镜像包有未保存的修改。继续导入会丢掉这些修改。要继续吗？"),
                        &discardUnsaved)) {
        return;
    }

    const QString source = QFileDialog::getOpenFileName(
        this, QStringLiteral("导入"), browseDirectory(),
        QStringLiteral("可导入的文件 (*.conf *.config *.bin);;"
                       "flash.conf (*.conf *.config);;"
                       "download.bin (*.bin);;"
                       "所有文件 (*)"),
        nullptr, QFileDialog::DontUseNativeDialog);
    if (source.isEmpty()) {
        return;
    }

    QString previewJson;
    QString error;
    if (!EtSession::previewImport(source, &previewJson, &error)) {
        showWarning(QStringLiteral("导入"), error);
        return;
    }
    ImportPreview preview;
    if (!ImportPreview::parse(previewJson, &preview, &error)) {
        showWarning(QStringLiteral("导入"), error);
        return;
    }
    ImportChoicesDialog dialog(QFileInfo(source).fileName(), preview, this);
    if (dialog.exec() != QDialog::Accepted) {
        return;
    }

    QString viewJson;
    if (!EtSession::importPackage(source, dialog.selectionJson(), &viewJson, &error)) {
        showWarning(QStringLiteral("导入"), error);
        return;
    }
    DocumentView view;
    if (!DocumentView::parse(viewJson, &view, &error)) {
        showWarning(QStringLiteral("导入"),
                    QStringLiteral("已写入当前镜像包，但界面没能读回结果：%1").arg(error));
        return;
    }
    applyDocument(view);
}

void MainWindow::savePackage() {
    saveToArchive(QString());
}

void MainWindow::savePackageAs() {
    if (!m_hasDocument) {
        showWarning(QStringLiteral("另存为"), QStringLiteral("请先新建或打开镜像包"));
        return;
    }
    const QString target = askArchivePath(QStringLiteral("另存为"));
    if (target.isEmpty()) {
        return;
    }
    QString viewJson;
    QString error;
    if (!EtSession::savePackageAs(target, &viewJson, &error)) {
        showWarning(QStringLiteral("另存为"), error);
        return;
    }
    DocumentView view;
    if (!DocumentView::parse(viewJson, &view, &error)) {
        showWarning(QStringLiteral("另存为"),
                    QStringLiteral("已写入 %1，但界面没能读回结果：%2").arg(target, error));
        return;
    }
    applyDocument(view);
}

void MainWindow::newPartition() {
    showPending(QStringLiteral("增加分区"));
}

void MainWindow::deletePartition() {
    if (m_partitionList == nullptr) {
        return;
    }
    const QStringList ids = m_partitionList->checkedPartitionIds();
    if (ids.isEmpty()) {
        return;
    }
    QString viewJson;
    QString error;
    if (!EtSession::removePartitions(ids, &viewJson, &error)) {
        showWarning(QStringLiteral("删除分区"), error);
        return;
    }
    DocumentView view;
    if (!DocumentView::parse(viewJson, &view, &error)) {
        showWarning(QStringLiteral("删除分区"),
                    QStringLiteral("分区已删除，但界面没能读回结果：%1").arg(error));
        return;
    }
    applyDocument(view);
}

void MainWindow::download() {
    showPending(QStringLiteral("下载"));
}

void MainWindow::onPartitionSelectionChanged(bool hasSelection) {
    if (m_partitionEditor == nullptr) {
        return;
    }
    if (!hasSelection || m_partitionList == nullptr) {
        m_partitionEditor->clearPartition();
        return;
    }
    const QString id = m_partitionList->selectedPartitionId();
    for (const PartitionView &partition : m_partitions) {
        if (partition.id == id) {
            m_partitionEditor->setPartition(partition, m_sectorSize);
            return;
        }
    }
    m_partitionEditor->clearPartition();
}

void MainWindow::onPartitionChecksChanged() {
    updateActionStates();
}
