#pragma once

#include <QIcon>

enum class ToolbarIcon {
    NewPackage,
    Save,
    AddPartition,
    DeletePartition,
    Download
};

QIcon toolbarIcon(ToolbarIcon kind);
