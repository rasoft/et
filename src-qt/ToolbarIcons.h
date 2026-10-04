#pragma once

#include <QIcon>

enum class ToolbarIcon {
    NewPackage,
    OpenPackage,
    Import,
    Save,
    AddPartition,
    DeletePartition,
    Download
};

QIcon toolbarIcon(ToolbarIcon kind);
