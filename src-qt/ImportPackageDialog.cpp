#include "ImportPackageDialog.h"

#include <QDialogButtonBox>
#include <QDir>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPushButton>
#include <QStandardPaths>
#include <QVBoxLayout>

namespace {

const char *kImportFilter =
    "可导入的文件 (*.conf *.config *.bin);;"
    "flash.conf (*.conf *.config);;"
    "download.bin (*.bin);;"
    "所有文件 (*)";

bool isSafePathComponent(const QString &name) {
    if (name.isEmpty() || name == QLatin1String(".") || name == QLatin1String("..")) {
        return false;
    }
    const QString forbidden = QStringLiteral("/\\:<>\"|?*");
    for (const QChar ch : name) {
        if (ch.unicode() < 0x20 || forbidden.contains(ch)) {
            return false;
        }
    }
    return true;
}

void showWarning(QWidget *parent, const QString &text) {
    QMessageBox box(parent);
    box.setIcon(QMessageBox::Warning);
    box.setWindowTitle(QStringLiteral("导入"));
    box.setText(text);
    box.addButton(QStringLiteral("确定"), QMessageBox::AcceptRole);
    box.exec();
}

} // namespace

QString ImportPackageDialog::pickSourceFile(QWidget *parent, const QString &startDirectory) {
    // Qt 自带对话框。Windows 7 上不用系统里较新的文件选择接口。
    return QFileDialog::getOpenFileName(parent, QStringLiteral("导入"), startDirectory,
                                        QString::fromUtf8(kImportFilter), nullptr,
                                        QFileDialog::DontUseNativeDialog);
}

ImportPackageDialog::ImportPackageDialog(const QString &sourceFile, QWidget *parent)
    : QDialog(parent) {
    setWindowTitle(QStringLiteral("导入"));
    setMinimumWidth(560);

    m_source = new QLineEdit(sourceFile, this);
    auto *sourceButton = new QPushButton(QStringLiteral("浏览..."), this);
    auto *sourceRow = new QWidget(this);
    auto *sourceLayout = new QHBoxLayout(sourceRow);
    sourceLayout->setContentsMargins(0, 0, 0, 0);
    sourceLayout->addWidget(m_source, 1);
    sourceLayout->addWidget(sourceButton);

    m_name = new QLineEdit(this);
    m_name->setPlaceholderText(QStringLiteral("例如 board-d1"));

    m_parent = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
    if (m_parent.isEmpty()) {
        m_parent = QDir::homePath();
    }
    m_path = new QLineEdit(this);
    auto *browseButton = new QPushButton(QStringLiteral("浏览..."), this);
    auto *pathRow = new QWidget(this);
    auto *pathLayout = new QHBoxLayout(pathRow);
    pathLayout->setContentsMargins(0, 0, 0, 0);
    pathLayout->addWidget(m_path, 1);
    pathLayout->addWidget(browseButton);

    auto *form = new QFormLayout;
    form->setFieldGrowthPolicy(QFormLayout::ExpandingFieldsGrow);
    form->setLabelAlignment(Qt::AlignRight | Qt::AlignVCenter);
    form->addRow(QStringLiteral("文件"), sourceRow);
    form->addRow(QStringLiteral("名称"), m_name);
    form->addRow(QStringLiteral("目录"), pathRow);

    auto *hint = new QLabel(
        QStringLiteral("可导入 flash.conf，或 genflash merge 产生的 download.bin。容量和扇区由文件决定。"),
        this);
    hint->setWordWrap(true);

    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    buttons->button(QDialogButtonBox::Ok)->setText(QStringLiteral("导入"));
    buttons->button(QDialogButtonBox::Cancel)->setText(QStringLiteral("取消"));

    auto *layout = new QVBoxLayout(this);
    layout->addLayout(form);
    layout->addWidget(hint);
    layout->addWidget(buttons);

    connect(sourceButton, &QPushButton::clicked, this, [this] { browseSource(); });
    connect(m_source, &QLineEdit::editingFinished, this, [this] { applySourceDefaults(); });
    connect(m_name, &QLineEdit::textEdited, this, [this] { m_nameEdited = true; });
    connect(m_name, &QLineEdit::textChanged, this, [this] { updateSuggestedPath(); });
    connect(m_path, &QLineEdit::textEdited, this, [this] { m_pathEdited = true; });
    connect(browseButton, &QPushButton::clicked, this, [this] { browseDirectory(); });
    connect(buttons, &QDialogButtonBox::accepted, this, [this] { accept(); });
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);

    applySourceDefaults();
    if (m_name->text().isEmpty()) {
        m_name->setFocus();
    }
}

QString ImportPackageDialog::sourceFile() const {
    return m_sourceFile;
}

QString ImportPackageDialog::packageName() const {
    return m_packageName;
}

QString ImportPackageDialog::directory() const {
    return m_directory;
}

void ImportPackageDialog::browseSource() {
    const QString start = m_source->text().trimmed().isEmpty()
                              ? m_parent
                              : QFileInfo(m_source->text()).absolutePath();
    const QString picked = pickSourceFile(this, start);
    if (picked.isEmpty()) {
        return;
    }
    m_source->setText(picked);
    applySourceDefaults();
}

void ImportPackageDialog::browseDirectory() {
    const QString picked = QFileDialog::getExistingDirectory(
        this, QStringLiteral("选择保存位置"), m_parent,
        QFileDialog::ShowDirsOnly | QFileDialog::DontUseNativeDialog);
    if (picked.isEmpty()) {
        return;
    }
    m_parent = picked;
    m_pathEdited = false;
    updateSuggestedPath();
}

void ImportPackageDialog::applySourceDefaults() {
    const QString source = m_source->text().trimmed();
    if (source.isEmpty()) {
        return;
    }
    const QFileInfo info(source);
    if (!m_nameEdited) {
        const QString base = info.completeBaseName().trimmed();
        if (!base.isEmpty()) {
            m_name->setText(base);
        }
    }
    if (!m_pathEdited && info.isFile()) {
        m_parent = info.absolutePath();
        updateSuggestedPath();
    }
}

void ImportPackageDialog::updateSuggestedPath() {
    if (m_pathEdited) {
        return;
    }
    const QString name = m_name->text().trimmed();
    if (!isSafePathComponent(name)) {
        m_path->clear();
        m_path->setPlaceholderText(QDir(m_parent).filePath(QStringLiteral("名称.etpk")));
        return;
    }
    m_path->setPlaceholderText({});
    m_path->setText(QDir(m_parent).filePath(name + QStringLiteral(".etpk")));
}

bool ImportPackageDialog::applyForm(QString *error) {
    const QString source = m_source->text().trimmed();
    if (source.isEmpty()) {
        *error = QStringLiteral("请选择要导入的文件");
        return false;
    }
    const QFileInfo sourceInfo(source);
    if (!sourceInfo.exists() || !sourceInfo.isFile()) {
        *error = QStringLiteral("请选择 flash.conf 或 download.bin 文件");
        return false;
    }

    const QString name = m_name->text().trimmed();
    if (name.isEmpty()) {
        *error = QStringLiteral("请填写名称");
        return false;
    }
    const QString typedPath = m_path->text().trimmed();
    if (typedPath.isEmpty()) {
        *error = isSafePathComponent(name)
                     ? QStringLiteral("请填写目录")
                     : QStringLiteral("名称不能包含 / \\ : * ? \" < > |");
        return false;
    }
    const QString path = QDir::cleanPath(QFileInfo(typedPath).absoluteFilePath());
    const QString leaf = QFileInfo(path).fileName();
    if (!isSafePathComponent(leaf)) {
        *error = QStringLiteral("目录名不能包含 / \\ : * ? \" < > |");
        return false;
    }
    const QFileInfo info(path);
    if (!info.dir().exists()) {
        *error = QStringLiteral("上级目录不存在");
        return false;
    }
    if (info.exists() && !info.isDir()) {
        *error = QStringLiteral("目标路径已存在，且不是目录");
        return false;
    }

    m_sourceFile = sourceInfo.absoluteFilePath();
    m_packageName = name;
    m_directory = path;
    return true;
}

void ImportPackageDialog::accept() {
    QString error;
    if (!applyForm(&error)) {
        showWarning(this, error);
        return;
    }
    QDialog::accept();
}
