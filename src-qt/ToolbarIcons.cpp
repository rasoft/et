#include "ToolbarIcons.h"

#include <QApplication>
#include <QColor>
#include <QIconEngine>
#include <QImage>
#include <QPainter>
#include <QPainterPath>
#include <QPalette>
#include <QPixmap>

namespace {

void paintDocument(QPainter *painter) {
    QPainterPath page;
    page.moveTo(3.4, 2.4);
    page.lineTo(8.8, 2.4);
    page.lineTo(8.8, 5.8);
    page.lineTo(12.6, 5.8);
    page.lineTo(12.6, 13.6);
    page.lineTo(3.4, 13.6);
    page.closeSubpath();
    painter->drawPath(page);
}

void paintFloppy(QPainter *painter) {
    painter->drawRoundedRect(QRectF(3.2, 2.4, 9.6, 11.4), 1.3, 1.3);
    painter->drawLine(QPointF(5.4, 2.4), QPointF(5.4, 6.0));
    painter->drawLine(QPointF(5.4, 6.0), QPointF(10.6, 6.0));
    painter->drawLine(QPointF(10.6, 6.0), QPointF(10.6, 2.4));
    painter->drawLine(QPointF(4.8, 9.3), QPointF(11.2, 9.3));
    painter->drawLine(QPointF(4.8, 11.5), QPointF(11.2, 11.5));
}

void paintPlus(QPainter *painter) {
    QPen pen = painter->pen();
    pen.setWidthF(1.7);
    painter->setPen(pen);
    painter->drawLine(QPointF(3.2, 8.0), QPointF(12.8, 8.0));
    painter->drawLine(QPointF(8.0, 3.2), QPointF(8.0, 12.8));
}

void paintTrash(QPainter *painter) {
    painter->drawLine(QPointF(6.3, 2.6), QPointF(9.7, 2.6));
    painter->drawLine(QPointF(6.3, 2.6), QPointF(6.3, 4.4));
    painter->drawLine(QPointF(9.7, 2.6), QPointF(9.7, 4.4));
    painter->drawLine(QPointF(3.2, 4.6), QPointF(12.8, 4.6));
    QPainterPath bin;
    bin.moveTo(4.3, 6.3);
    bin.lineTo(5.2, 13.6);
    bin.lineTo(10.8, 13.6);
    bin.lineTo(11.7, 6.3);
    painter->drawPath(bin);
    painter->drawLine(QPointF(8.0, 7.7), QPointF(8.0, 11.7));
}

void paintDownload(QPainter *painter) {
    painter->drawLine(QPointF(8.0, 2.0), QPointF(8.0, 8.0));
    painter->drawLine(QPointF(4.9, 5.2), QPointF(8.0, 8.3));
    painter->drawLine(QPointF(11.1, 5.2), QPointF(8.0, 8.3));
    QPainterPath tray;
    tray.moveTo(3.3, 10.6);
    tray.lineTo(3.3, 13.8);
    tray.lineTo(12.7, 13.8);
    tray.lineTo(12.7, 10.6);
    painter->drawPath(tray);
}

class ToolbarIconEngine : public QIconEngine {
public:
    explicit ToolbarIconEngine(ToolbarIcon kind)
        : m_kind(kind) {}

    QIconEngine *clone() const override { return new ToolbarIconEngine(m_kind); }

    QPixmap pixmap(const QSize &size, QIcon::Mode mode, QIcon::State state) override {
        QImage image(size, QImage::Format_ARGB32_Premultiplied);
        image.fill(Qt::transparent);
        QPainter painter(&image);
        paint(&painter, QRect(QPoint(0, 0), size), mode, state);
        return QPixmap::fromImage(image);
    }

    void paint(QPainter *painter, const QRect &rect, QIcon::Mode mode, QIcon::State state) override {
        Q_UNUSED(state);
        if (rect.isEmpty()) {
            return;
        }
        const QPalette::ColorGroup group =
            mode == QIcon::Disabled ? QPalette::Disabled : QPalette::Active;
        const QColor color = QApplication::palette().color(group, QPalette::ButtonText);

        painter->save();
        painter->setRenderHint(QPainter::Antialiasing, true);
        const qreal side = qMin(rect.width(), rect.height());
        painter->translate(rect.center());
        painter->scale(side / 16.0, side / 16.0);
        painter->translate(-8.0, -8.0);
        painter->setPen(QPen(color, 1.45, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
        painter->setBrush(Qt::NoBrush);
        switch (m_kind) {
        case ToolbarIcon::NewPackage:
            paintDocument(painter);
            break;
        case ToolbarIcon::Save:
            paintFloppy(painter);
            break;
        case ToolbarIcon::AddPartition:
            paintPlus(painter);
            break;
        case ToolbarIcon::DeletePartition:
            paintTrash(painter);
            break;
        case ToolbarIcon::Download:
            paintDownload(painter);
            break;
        }
        painter->restore();
    }

private:
    ToolbarIcon m_kind;
};

} // namespace

QIcon toolbarIcon(ToolbarIcon kind) {
    return QIcon(new ToolbarIconEngine(kind));
}
