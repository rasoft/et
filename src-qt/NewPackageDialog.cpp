#include "NewPackageDialog.h"

#include <QComboBox>
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
#include <QRegularExpression>
#include <QRegularExpressionValidator>
#include <QStandardPaths>
#include <QVBoxLayout>

#include <limits>

namespace {

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

bool parseU64(const QString &text, quint64 *out) {
    if (text.isEmpty()) {
        return false;
    }
    quint64 value = 0;
    for (const QChar ch : text) {
        if (!ch.isDigit()) {
            return false;
        }
        const int digit = ch.digitValue();
        if (digit < 0) {
            return false;
        }
        if (value > (std::numeric_limits<quint64>::max() - static_cast<quint64>(digit)) / 10) {
            return false;
        }
        value = value * 10 + static_cast<quint64>(digit);
    }
    *out = value;
    return true;
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

NewPackageDialog::NewPackageDialog(QWidget *parent)
    : QDialog(parent) {
    setWindowTitle(QStringLiteral("新建镜像包"));
    setMinimumWidth(520);

    m_name = new QLineEdit(this);
    m_name->setPlaceholderText(QStringLiteral("例如 board-d1"));

    m_capacity = new QLineEdit(QStringLiteral("16"), this);
    m_capacity->setValidator(new QRegularExpressionValidator(
        QRegularExpression(QStringLiteral("[0-9]{0,20}")), m_capacity));
    m_unit = new QComboBox(this);
    m_unit->addItem(QStringLiteral("GiB"), 30);
    m_unit->addItem(QStringLiteral("MiB"), 20);
    m_unit->addItem(QStringLiteral("KiB"), 10);
    m_unit->addItem(QStringLiteral("字节"), 0);

    auto *capacityRow = new QWidget(this);
    auto *capacityLayout = new QHBoxLayout(capacityRow);
    capacityLayout->setContentsMargins(0, 0, 0, 0);
    capacityLayout->addWidget(m_capacity, 1);
    capacityLayout->addWidget(m_unit);

    m_sector = new QComboBox(this);
    m_sector->addItem(QStringLiteral("512 字节"), 512);
    m_sector->addItem(QStringLiteral("4096 字节"), 4096);

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
    form->addRow(QStringLiteral("名称"), m_name);
    form->addRow(QStringLiteral("容量"), capacityRow);
    form->addRow(QStringLiteral("扇区大小"), m_sector);
    form->addRow(QStringLiteral("目录"), pathRow);

    auto *hint = new QLabel(QStringLiteral("将创建该目录，并写入 manifest.json 和空的 images/。"), this);
    hint->setWordWrap(true);

    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    buttons->button(QDialogButtonBox::Ok)->setText(QStringLiteral("创建"));
    buttons->button(QDialogButtonBox::Cancel)->setText(QStringLiteral("取消"));

    auto *layout = new QVBoxLayout(this);
    layout->addLayout(form);
    layout->addWidget(hint);
    layout->addWidget(buttons);

    connect(m_name, &QLineEdit::textChanged, this, [this] { updateSuggestedPath(); });
    connect(m_path, &QLineEdit::textEdited, this, [this] { m_pathEdited = true; });
    connect(browseButton, &QPushButton::clicked, this, [this] { browse(); });
    connect(buttons, &QDialogButtonBox::accepted, this, [this] { accept(); });
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);

    updateSuggestedPath();
    m_name->setFocus();
}

QString NewPackageDialog::packageName() const {
    return m_packageName;
}

QString NewPackageDialog::directory() const {
    return m_directory;
}

quint64 NewPackageDialog::userAreaBytes() const {
    return m_userAreaBytes;
}

quint32 NewPackageDialog::sectorSize() const {
    return m_sectorSize;
}

void NewPackageDialog::updateSuggestedPath() {
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

void NewPackageDialog::browse() {
    // Qt 自带对话框。Windows 7 上不用系统里较新的文件选择接口。
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

bool NewPackageDialog::applyForm(QString *error) {
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

    quint64 magnitude = 0;
    if (!parseU64(m_capacity->text().trimmed(), &magnitude) || magnitude == 0) {
        *error = QStringLiteral("请填写大于 0 的容量");
        return false;
    }
    const int shift = m_unit->currentData().toInt();
    if (shift < 0 || shift >= 64
        || (shift > 0 && magnitude > (std::numeric_limits<quint64>::max() >> shift))) {
        *error = QStringLiteral("容量太大");
        return false;
    }
    const quint32 sector = m_sector->currentData().toUInt();
    if (sector != 512 && sector != 4096) {
        *error = QStringLiteral("扇区大小只能是 512 或 4096");
        return false;
    }

    m_packageName = name;
    m_directory = path;
    m_userAreaBytes = magnitude << shift;
    m_sectorSize = sector;
    return true;
}

void NewPackageDialog::accept() {
    QString error;
    if (!applyForm(&error)) {
        showWarning(this, error);
        return;
    }
    QDialog::accept();
}
