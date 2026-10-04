#pragma once

#include <QIcon>

enum class ToolbarIcon {
    NewPackage,
    OpenPackage,
    Save,
    AddPartition,
    DeletePartition,
    Download
};

QIcon toolbarIcon(ToolbarIcon kind);
